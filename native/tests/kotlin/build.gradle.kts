plugins {
    kotlin("jvm") version "2.1.20"
}
repositories { mavenCentral() }
dependencies {
    implementation("net.java.dev.jna:jna:5.16.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.10.2")
    testImplementation(kotlin("test-junit"))
}
kotlin {
    jvmToolchain(17)
    sourceSets.main { kotlin.srcDir("../../generated/kotlin") }
}
tasks.test {
    systemProperty("jna.library.path", file("../../target/release").absolutePath)
    testLogging {
        events("passed", "skipped", "failed")
        exceptionFormat = org.gradle.api.tasks.testing.logging.TestExceptionFormat.FULL
    }
}
