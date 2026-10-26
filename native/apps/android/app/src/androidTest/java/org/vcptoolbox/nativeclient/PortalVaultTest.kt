package org.vcptoolbox.nativeclient

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.security.KeyStore
import java.util.UUID
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.vcptoolbox.nativecore.NativeSessionInfo
import org.vcptoolbox.nativecore.SessionCredential
import org.vcptoolbox.nativecore.credentialStorageKey
import org.vcptoolbox.nativecore.NativePendingKind
import org.vcptoolbox.nativecore.NativeRecoveryFence
import org.vcptoolbox.nativecore.NativeRecoveryException
import org.vcptoolbox.nativecore.SessionCredentialException
import kotlinx.coroutines.delay

@RunWith(AndroidJUnit4::class)
class PortalVaultTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val prefix = UUID.randomUUID().toString()
    private val a = "https://a-$prefix.example.test"
    private val b = "https://b-$prefix.example.test"
    private val vault = PortalSessionVault(context)
    private fun issued(origin: String, token: String = "A".repeat(43)): SessionCredential =
        SessionCredential.fromServerCookie(origin, origin,
            "__Secure-vcp_mobile=$token; Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age=600")
    private fun file(origin: String) = File(context.noBackupFilesDir, "portal-session-v1/${credentialStorageKey(origin)}.bin")
    private suspend fun rejected(action: suspend () -> Unit) {
        var rejected = false
        try { action() }
        catch (expected: PortalVaultException) {
            rejected = true
            assertFalse(expected.message.orEmpty().contains("A".repeat(43)))
        }
        assertTrue("Expected a redacted vault rejection", rejected)
    }
    @After fun cleanup(): Unit = runBlocking {
        for (origin in listOf(a,b,"$a:8443")) {
            vault.remove(origin)
            File(context.noBackupFilesDir,"portal-pending-v1/${credentialStorageKey(origin)}").deleteRecursively()
            for (suffix in listOf("", "-wal", "-shm")) {
                File(context.noBackupFilesDir,"portal-pending-v1/vcp-pending-v1-${credentialStorageKey(origin)}.sqlite$suffix").delete()
            }
        }
    }
    @Test fun encryptedRoundtripAcrossInstances(): Unit = runBlocking {
        val original = issued(a)
        try {
            vault.save(original)
            assertFalse(file(a).readBytes().toString(Charsets.UTF_8).contains("A".repeat(43)))
            val restored = PortalSessionVault(context).load(a)
            assertNotNull(restored)
            try {
                assertArrayEquals(original.secretBytesForVault(), restored!!.secretBytesForVault())
                assertEquals(original.expiresAtMs(), restored.expiresAtMs())
            } finally { restored?.destroy() }
        } finally { original.destroy() }
    }
    @Test fun canonicalOriginAndPortIsolation(): Unit = runBlocking {
        val original = issued(a); val port = issued("$a:8443", "B".repeat(43))
        try {
            vault.save(original); vault.save(port)
            val restored = vault.load(a.uppercase().replace("HTTPS://", "https://") + ":443")
            try { assertEquals(original.expiresAtMs(), restored!!.expiresAtMs()) }
            finally { restored?.destroy() }
            assertNull(vault.load(b))
            assertNotEquals(credentialStorageKey(a), credentialStorageKey("$a:8443"))
        } finally { original.destroy(); port.destroy() }
    }
    @Test fun tamperedCiphertextFailsWithoutDeletingRecord(): Unit = runBlocking {
        val original = issued(a)
        try { vault.save(original) } finally { original.destroy() }
        val bytes = file(a).readBytes()
        bytes[bytes.lastIndex] = (bytes.last().toInt() xor 1).toByte()
        file(a).writeBytes(bytes)
        rejected { vault.load(a) }
        assertArrayEquals(bytes, file(a).readBytes())
    }
    @Test fun ciphertextCannotMoveToAnotherOrigin(): Unit = runBlocking {
        val original = issued(a); val other = issued(b)
        try { vault.save(original); vault.save(other) } finally { original.destroy(); other.destroy() }
        file(b).writeBytes(file(a).readBytes())
        rejected { vault.load(b) }
        assertTrue(file(b).exists())
    }
    @Test fun missingKeyNeverRegeneratesOrResends(): Unit = runBlocking {
        val original = issued(a)
        try { vault.save(original) } finally { original.destroy() }
        val id = credentialStorageKey(a)
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        store.deleteEntry("vcp.mobile.session.v1.$id")
        val before = file(a).readBytes()
        rejected { vault.load(a) }
        assertFalse(store.containsAlias("vcp.mobile.session.v1.$id"))
        assertArrayEquals(before, file(a).readBytes())
    }
    @Test fun removalIsOriginScopedAndUnpairedFetchDoesNotUseNetwork(): Unit = runBlocking {
        val original = issued(a); val other = issued(b)
        try { vault.save(original); vault.save(other) } finally { original.destroy(); other.destroy() }
        vault.remove(a)
        assertNull(vault.load(a))
        val remaining = vault.load(b)
        try { assertNotNull(remaining) } finally { remaining?.destroy() }
        rejected { vault.fetchSession(a) }
        rejected { vault.load("http://example.test") }
    }
    @Test fun unpairedHistoryDoesNotUseNetwork(): Unit = runBlocking {
        val expected = NativeSessionInfo("a".repeat(64), "b".repeat(64), listOf("topic_a"), null)
        try {
            vault.fetchHistory(a, expected, "topic_a", 0uL)
            fail("Unpaired history must not reach HTTPS")
        } catch (failure: PortalVaultException) {
            assertEquals("NOT_PAIRED", failure.code)
        }
        assertFalse(file(a).exists())
    }

    @Test fun unpairedRecoveryDoesNotUseNetwork(): Unit = runBlocking {
        val expected = NativeSessionInfo("a".repeat(64), "b".repeat(64), listOf("topic_a"), null)
        val fence = NativeRecoveryFence(expected,"topic_a",1uL)
        try {
            vault.recoverPending(a,expected,"topic_a",1uL,fence)
            fail("Unpaired recovery must not reach HTTPS")
        } catch (failure: PortalVaultException) { assertEquals("NOT_PAIRED",failure.code) }
        finally { fence.destroy() }
        assertFalse(file(a).exists())
    }
    @Test fun pendingCheckpointSurvivesReopenCancellationAndCredentialRemoval(): Unit = runBlocking {
        val original = issued(a)
        try { vault.save(original) } finally { original.destroy() }
        val expected = NativeSessionInfo("a".repeat(64),"b".repeat(64),listOf("topic_a"),null)
        val pending = vault.checkpointOriginal(a,expected,"topic_a",NativePendingKind.MESSAGE,
            "{ \"requestId\":\"native_pending_123\", \"text\":\"synthetic pending\" }")
        val reopened = PortalSessionVault(context)
        assertEquals(listOf(pending),reopened.pendingOriginals(a,"topic_a"))
        val fence = NativeRecoveryFence(expected,"topic_a",2uL)
        fence.cancel()
        try {
            reopened.recoverPending(a,expected,"topic_a",2uL,fence)
            fail("Cancelled recovery must not reach HTTPS")
        } catch (_: NativeRecoveryException.Cancelled) {}
        finally { fence.destroy() }
        assertEquals(listOf(pending),reopened.pendingOriginals(a,"topic_a"))
        vault.remove(a)
        assertEquals(listOf(pending),reopened.pendingOriginals(a,"topic_a"))
        File(context.noBackupFilesDir,"portal-pending-v1/${credentialStorageKey(a)}").deleteRecursively()
    }
    @Test fun expiredRecoveryRetainsCredentialAndPending(): Unit = runBlocking {
        val original = SessionCredential.fromServerCookie(a,a,
            "__Secure-vcp_mobile=${"B".repeat(43)}; Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age=5")
        try { vault.save(original) } finally { original.destroy() }
        val expected = NativeSessionInfo("a".repeat(64),"b".repeat(64),listOf("topic_a"),null)
        val pending = vault.checkpointOriginal(a,expected,"topic_a",NativePendingKind.MESSAGE,
            "{\"requestId\":\"expired_pending_123\",\"text\":\"synthetic\"}")
        val before = file(a).readBytes()
        delay(5100)
        val fence = NativeRecoveryFence(expected,"topic_a",3uL)
        try {
            vault.recoverPending(a,expected,"topic_a",3uL,fence)
            fail("Expired recovery must not reach HTTPS")
        } catch (_: SessionCredentialException.Expired) {}
        finally { fence.destroy() }
        assertArrayEquals(before,file(a).readBytes())
        assertEquals(listOf(pending),vault.pendingOriginals(a,"topic_a"))
    }

}
