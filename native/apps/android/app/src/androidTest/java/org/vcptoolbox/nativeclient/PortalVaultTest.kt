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
    @After fun cleanup(): Unit = runBlocking { vault.remove(a); vault.remove(b); vault.remove("$a:8443") }
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

}
