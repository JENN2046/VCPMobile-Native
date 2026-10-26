package org.vcptoolbox.nativeclient

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.AtomicFile
import java.io.File
import java.io.FileNotFoundException
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.vcptoolbox.nativecore.PairingAttempt
import org.vcptoolbox.nativecore.NativeHistoryPage
import org.vcptoolbox.nativecore.NativeSessionInfo
import org.vcptoolbox.nativecore.SessionCredential
import org.vcptoolbox.nativecore.NativePendingJournal
import org.vcptoolbox.nativecore.NativePendingPreview
import org.vcptoolbox.nativecore.credentialStorageKey

/** Fixed error categories; do not leak sensitive system error messages. */
internal class PortalVaultException(val code: String) : Exception("Session vault: $code")

/** Per-origin AndroidKeyStore AES/GCM with app-private no-backup AtomicFile.
 * All crypto and IO is off Main. Multiple instances in one process share gate.
 * Missing/corrupt keys never trigger silent regeneration over an existing record.
 */
internal class PortalSessionVault(context: Context) {
    private val directory = File(context.applicationContext.noBackupFilesDir, "portal-session-v1")
    private val pendingDirectory = File(context.applicationContext.noBackupFilesDir, "portal-pending-v1")
    companion object {
        private val gate = Any()
        private val epochs = mutableMapOf<String, Long>()
        private const val MAX_BYTES = 4096
        private const val PREFIX = "vcp.mobile.session.v1."
        private val magic = byteArrayOf(86, 67, 80, 1)
    }
    private fun keyId(origin: String): String = try { credentialStorageKey(origin) }
        catch (_: Exception) { throw PortalVaultException("INVALID_ORIGIN") }
    private fun file(id: String) = AtomicFile(File(directory, "$id.bin"))
    private fun alias(id: String) = PREFIX + id
    private fun keyStore() = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
    private fun aad(id: String) = ("VCPMobile.SessionVault.v1\u0000" + id).toByteArray(Charsets.UTF_8)
    private fun bump(id: String) { epochs[id] = (epochs[id] ?: 0L) + 1L }
    private fun readBytes(record: AtomicFile): ByteArray? {
        try {
            record.openRead().use { input ->
                val buffer = ByteArray(MAX_BYTES + 1)
                var size = 0
                while (size < buffer.size) {
                    val read = input.read(buffer, size, buffer.size - size)
                    if (read < 0) break
                    if (read == 0) throw PortalVaultException("READ_FAILED")
                    size += read
                }
                if (size > MAX_BYTES) throw PortalVaultException("CORRUPT")
                return buffer.copyOf(size)
            }
        } catch (_: FileNotFoundException) {
            if (record.baseFile.exists() || File(record.baseFile.path + ".bak").exists()) {
                throw PortalVaultException("READ_FAILED")
            }
            return null
        }
    }
    private fun existingKey(id: String): SecretKey? =
        keyStore().getKey(alias(id), null) as? SecretKey
    private fun newKey(id: String): SecretKey {
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        generator.init(KeyGenParameterSpec.Builder(alias(id), KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
            .setKeySize(256).setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setRandomizedEncryptionRequired(true).build())
        return generator.generateKey()
    }
    private fun loadLocked(origin: String, id: String): SessionCredential? {
        val bytes = readBytes(file(id)) ?: return null
        if (bytes.size < 32 || !bytes.copyOfRange(0, 4).contentEquals(magic)) {
            throw PortalVaultException("CORRUPT")
        }
        val key = existingKey(id) ?: throw PortalVaultException("KEY_UNAVAILABLE")
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(128, bytes.copyOfRange(4, 16)))
        cipher.updateAAD(aad(id))
        val plain = cipher.doFinal(bytes, 16, bytes.size - 16)
        try { return SessionCredential.restore(origin, plain) }
        finally { plain.fill(0) }
    }
    suspend fun save(credential: SessionCredential) = saveChecked(credential, null)
    private suspend fun saveChecked(credential: SessionCredential, expectedEpoch: Long?) = withContext(Dispatchers.IO) {
        synchronized(gate) {
            try {
                val id = credential.storageKey()
                if (expectedEpoch != null && (epochs[id] ?: 0L) != expectedEpoch) {
                    throw PortalVaultException("SESSION_CHANGED")
                }
                val plain = credential.secretBytesForVault()
                try {
                    if (plain.size > 2048) throw PortalVaultException("INVALID_RECORD")
                    if (!directory.isDirectory && !directory.mkdirs()) throw PortalVaultException("WRITE_FAILED")
                    val record = file(id)
                    val key = existingKey(id) ?: run {
                        if (readBytes(record) != null) throw PortalVaultException("KEY_UNAVAILABLE")
                        newKey(id)
                    }
                    val cipher = Cipher.getInstance("AES/GCM/NoPadding")
                    cipher.init(Cipher.ENCRYPT_MODE, key)
                    if (cipher.iv.size != 12) throw PortalVaultException("CRYPTO_UNAVAILABLE")
                    cipher.updateAAD(aad(id))
                    val encrypted = magic + cipher.iv + cipher.doFinal(plain)
                    val stream = record.startWrite()
                    try {
                        stream.write(encrypted)
                        record.finishWrite(stream)
                    } catch (failure: Exception) {
                        record.failWrite(stream)
                        throw failure
                    }
                    // A post-commit read error refuses success; never auto-resends.
                    if (readBytes(record)?.contentEquals(encrypted) != true) {
                        throw PortalVaultException("WRITE_VERIFY_FAILED")
                    }
                    bump(id)
                } finally { plain.fill(0) }
            } catch (failure: PortalVaultException) { throw failure }
            catch (_: Exception) { throw PortalVaultException("WRITE_FAILED") }
        }
    }
    suspend fun load(origin: String): SessionCredential? = withContext(Dispatchers.IO) {
        synchronized(gate) {
            try { loadLocked(origin, keyId(origin)) }
            catch (failure: PortalVaultException) { throw failure }
            catch (_: Exception) { throw PortalVaultException("READ_REJECTED") }
        }
    }
    /** Local deletion only; not server logout, task cancel, or pending cleanup. */
    suspend fun remove(origin: String) = withContext(Dispatchers.IO) {
        synchronized(gate) {
            val id = keyId(origin)
            bump(id)
            try {
                val record = file(id)
                record.delete()
                if (record.baseFile.exists() || File(record.baseFile.path + ".bak").exists()) {
                    throw PortalVaultException("DELETE_FAILED")
                }
                keyStore().deleteEntry(alias(id))
            } catch (failure: PortalVaultException) { throw failure }
            catch (_: Exception) { throw PortalVaultException("DELETE_FAILED") }
        }
    }
    /** Explicit one-use pairing. A late response cannot undo removal/new pairing.
     * Save first, query later. Never retries a consumed code after a failed GET.
     */
    suspend fun pairAndSave(origin: String, code: String) {
        val operation = PairingAttempt(origin, code)
        try {
            val id = keyId(origin)
            val epoch = withContext(Dispatchers.IO) {
                synchronized(gate) { bump(id); epochs.getValue(id) }
            }
            val credential = withContext(Dispatchers.Default) { operation.pair() }
            try { saveChecked(credential, epoch) }
            finally { credential.destroy() }
        } finally { operation.destroy() }
    }
    /** Only a deliberate caller can issue GET. Never launch automatically. */
    suspend fun fetchSession(origin: String): NativeSessionInfo {
        val id = keyId(origin)
        val snapshot = withContext(Dispatchers.IO) {
            synchronized(gate) {
                try {
                    Pair(loadLocked(origin, id) ?: throw PortalVaultException("NOT_PAIRED"), epochs[id] ?: 0L)
                } catch (failure: PortalVaultException) { throw failure }
                catch (_: Exception) { throw PortalVaultException("READ_REJECTED") }
            }
        }
        try {
            val result = withContext(Dispatchers.Default) { snapshot.first.fetchSessionInfo() }
            synchronized(gate) {
                if ((epochs[id] ?: 0L) != snapshot.second) throw PortalVaultException("SESSION_CHANGED")
            }
            return result
        } finally { snapshot.first.destroy() }
    }
    /** Explicit history GET with a protected credential snapshot. View/Topic
     * generation must also be checked by the caller before displaying a page.
     */
    suspend fun fetchHistory(origin: String, expected: NativeSessionInfo,
                             topicKey: String, after: ULong): NativeHistoryPage {
        val id = keyId(origin)
        val snapshot = withContext(Dispatchers.IO) {
            synchronized(gate) {
                try {
                    Pair(loadLocked(origin, id) ?: throw PortalVaultException("NOT_PAIRED"), epochs[id] ?: 0L)
                } catch (failure: PortalVaultException) { throw failure }
                catch (_: Exception) { throw PortalVaultException("READ_REJECTED") }
            }
        }
        try {
            val page = withContext(Dispatchers.Default) {
                snapshot.first.fetchHistoryPage(expected, topicKey, after)
            }
            synchronized(gate) {
                if ((epochs[id] ?: 0L) != snapshot.second) throw PortalVaultException("SESSION_CHANGED")
            }
            return page
        } finally { snapshot.first.destroy() }
    }


    /** Explicit authenticated local recovery inventory. A live session GET is
     * required, while original payloads never cross into the UI. No POST/cleanup.
     * The credential epoch is verified AFTER the network call.
     */
    suspend fun listPending(origin: String, expected: NativeSessionInfo,
                            topic: String): List<NativePendingPreview> {
        val id = keyId(origin)
        val snapshot = withContext(Dispatchers.IO) {
            synchronized(gate) {
                try {
                    Pair(loadLocked(origin, id) ?: throw PortalVaultException("NOT_PAIRED"),
                         epochs[id] ?: 0L)
                } catch (failure: PortalVaultException) { throw failure }
                catch (_: Exception) { throw PortalVaultException("READ_REJECTED") }
            }
        }
        try {
            val previews = withContext(Dispatchers.IO) {
                if (!pendingDirectory.isDirectory && !pendingDirectory.mkdirs()) {
                    throw PortalVaultException("PENDING_STORE_UNAVAILABLE")
                }
                val journal = NativePendingJournal(origin, pendingDirectory.absolutePath)
                try { journal.listForTopic(snapshot.first, expected, topic) }
                finally { journal.destroy() }
            }
            synchronized(gate) {
                if ((epochs[id] ?: 0L) != snapshot.second) throw PortalVaultException("SESSION_CHANGED")
            }
            return previews
        } finally { snapshot.first.destroy() }
    }
}
