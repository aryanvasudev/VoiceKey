package com.voicekey.app

import com.voicekey.app.model.VoiceKeyModelManager
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

class VoiceKeyModelManagerTest {

    @get:Rule
    val tempFolder = TemporaryFolder()

    @Test
    fun testSha256ChecksumCalculation() {
        val testFile = tempFolder.newFile("test_sample.txt")
        testFile.writeText("hello")

        val hash = VoiceKeyModelManager.calculateSha256(testFile)
        // Known SHA-256 of "hello"
        assertEquals("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824", hash)
    }

    @Test
    fun testSha256EmptyFile() {
        val emptyFile = tempFolder.newFile("empty.bin")
        val hash = VoiceKeyModelManager.calculateSha256(emptyFile)
        // Known SHA-256 of empty file
        assertEquals("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", hash)
    }

    @Test
    fun testPresetsRegistry() {
        val presets = VoiceKeyModelManager.PRESETS
        assertEquals(3, presets.size)

        val tiny = presets.first { it.id == "tiny.en" }
        assertEquals("ggml-tiny.en.bin", tiny.filename)
        assertEquals("bd577a113a864445d4c299885e83797195274dbed8d2426916ac333e6c0a0636", tiny.expectedSha256)
        assertTrue(tiny.sizeBytes > 70_000_000L)

        val base = presets.first { it.id == "base.en" }
        assertEquals("ggml-base.en.bin", base.filename)
        assertEquals("465707469f7c0208fc1e1bd187bb6144eb5a9bf87d7f22bde5774022f641dcdd", base.expectedSha256)

        val small = presets.first { it.id == "small.en" }
        assertEquals("ggml-small.en.bin", small.filename)
        assertEquals("55356645c2b361a969dfd0ef2c5a50d530abb21a16f4cad4736f9bcb800cc441", small.expectedSha256)
    }
}
