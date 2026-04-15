import com.vanniktech.maven.publish.AndroidSingleVariantLibrary
import org.jetbrains.kotlin.gradle.dsl.JvmTarget
import org.jetbrains.kotlin.gradle.plugin.mpp.apple.XCFramework

plugins {
    alias(libs.plugins.androidLibrary)
    alias(libs.plugins.kotlinMultiplatform)
    alias(libs.plugins.kotlinSerialization)
    alias(libs.plugins.atomicfu)
    alias(libs.plugins.mavenPublish)
    signing
}

kotlin {
    androidTarget {
        publishLibraryVariants("release")
        compilerOptions { jvmTarget.set(JvmTarget.JVM_1_8) }
    }

    jvm()

    val xcf = XCFramework()
    listOf(
        iosArm64(),
        iosSimulatorArm64()
    ).forEach {

        it.binaries.framework {
            baseName = "lwk"
            xcf.add(this)
        }

        val platform = when (it.targetName) {
            "iosSimulatorArm64" -> "ios_simulator_arm64"
            "iosArm64" -> "ios_arm64"
            else -> error("Unsupported target $name")
        }


        it.compilations["main"].cinterops {
            create("lwkCInterop") {
                defFile(project.file("src/nativeInterop/cinterop/lwk.def"))
                includeDirs(project.file("src/nativeInterop/cinterop/headers/lwk/"), project.file("src/libs/$platform"))
            }
        }
    }

    compilerOptions.freeCompilerArgs.add("-Xexpect-actual-classes")

    sourceSets {
        commonMain.dependencies {
            implementation(libs.kotlinx.coroutines.core)
            implementation(libs.kotlinx.serialization.core)
            implementation(libs.kotlinx.serialization.json)
            implementation(libs.annotation)
            implementation(libs.okio)
        }
        commonTest.dependencies {
            implementation(kotlin("test"))
        }
        androidMain.dependencies {
            implementation(libs.jna.get()) {
                artifact { type = "aar" }
            }
        }
        jvmMain.dependencies {
            implementation(libs.jna)
        }
        androidUnitTest.dependencies {
            implementation(libs.junit)
        }
        androidInstrumentedTest.dependencies {
            implementation(libs.junit.ext)
            implementation(libs.espresso.core)
        }
    }

}

android {
    namespace = "com.blockstream.lwk_bindings"
    compileSdk = 36

    defaultConfig {
        minSdk = 24

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        consumerProguardFiles("consumer-rules.pro")
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }
}

val libraryVersion: String by project

val publishUsesGradleCoordinates = listOf("GROUP", "POM_ARTIFACT_ID", "VERSION_NAME")
    .any { providers.gradleProperty(it).orNull != null }
val publishUsesGradlePomMetadata = listOf(
    "POM_NAME",
    "POM_DESCRIPTION",
    "POM_URL",
    "POM_LICENSE_NAME",
    "POM_LICENSE_URL",
    "POM_LICENSE_DIST",
    "POM_DEVELOPER_ID",
    "POM_DEVELOPER_NAME",
    "POM_DEVELOPER_EMAIL",
    "POM_DEVELOPER_URL",
    "POM_SCM_URL",
    "POM_SCM_CONNECTION",
    "POM_SCM_DEV_CONNECTION"
).any { providers.gradleProperty(it).orNull != null }
val publishHasLicenseMetadata = listOf(
    "POM_LICENSE_NAME",
    "POM_LICENSE_URL",
    "POM_LICENSE_DIST"
).all { providers.gradleProperty(it).orNull != null }
val publishRepositoryUrl = providers.gradleProperty("mavenPublishRepositoryUrl").orNull
val publishRepositoryUsername = providers.gradleProperty("mavenPublishUsername")
    .orElse(providers.gradleProperty("mavenCentralUsername"))
    .orNull
val publishRepositoryPassword = providers.gradleProperty("mavenPublishPassword")
    .orElse(providers.gradleProperty("mavenCentralPassword"))
    .orNull
val publishCustomRepositoryUrl = publishRepositoryUrl
val publishRequireSigning = providers.gradleProperty("mavenPublishRequireSigning")
    .map(String::toBoolean)
    .orElse(publishCustomRepositoryUrl == null)
    .get()
val signingInMemoryKey = providers.gradleProperty("signingInMemoryKey").orNull
val signingInMemoryKeyPassword = providers.gradleProperty("signingInMemoryKeyPassword").orNull

mavenPublishing {
    if (!publishUsesGradleCoordinates) {
        coordinates(groupId = "com.blockstream", artifactId = "lwk", version = libraryVersion)
    }

    if (!publishUsesGradlePomMetadata) {
        pom {
            name = "LWK"
            description = "Liquid Wallet Kit"
            url = "https://blockstream.com"
            licenses {
                license {
                    name = "BSD-MIT"
                    url = "https://github.com/blockstream/lwk/blob/main/LICENSE"
                    distribution = "repo"
                }
            }
            developers {
                developer {
                    id = "rcasatta"
                    name = "Riccardo Casatta"
                    email = "riccardo@blockstream.com"
                }
                developer {
                    id = "leocomandini"
                    name = "Leonardo Comandini"
                    email = "leonardo@blockstream.com"
                }
            }
            scm {
                connection = "scm:git:github.com/blockstream/lwk.git"
                developerConnection = "scm:git:ssh://github.com/blockstream/lwk.git"
                url = "https://github.com/blockstream/lwk"
            }
        }
    }
    if (!publishHasLicenseMetadata) {
        pom {
            licenses {
                license {
                    name = "BSD-MIT"
                    url = "https://github.com/blockstream/lwk/blob/main/LICENSE"
                    distribution = "repo"
                }
            }
        }
    }

    if (publishCustomRepositoryUrl == null) {
        publishToMavenCentral()
    }
    if (publishRequireSigning) {
        signAllPublications()
    }
}

if (publishCustomRepositoryUrl != null) {
    publishing {
        repositories {
            maven {
                name = "lwkCustomRepository"
                url = uri(publishCustomRepositoryUrl)
                if (publishRepositoryUsername != null || publishRepositoryPassword != null) {
                    credentials {
                        username = publishRepositoryUsername
                        password = publishRepositoryPassword
                    }
                }
            }
        }
    }
}

if (publishRequireSigning) {
    extensions.configure<SigningExtension> {
        if (signingInMemoryKey != null) {
            useInMemoryPgpKeys(signingInMemoryKey, signingInMemoryKeyPassword)
        } else {
            useGpgCmd()
        }
    }
}

// Do not require signing when publishing to Maven Local
// Allows `./gradlew publishToMavenLocal` (or `publishToLocalMaven`) without GPG setup
tasks.withType<Sign>().configureEach {
    onlyIf {
        val taskNames = gradle.startParameter.taskNames
        // Skip signing if the build is targeting the local Maven repository
        publishRequireSigning &&
            taskNames.none {
                it.contains("publishToMavenLocal", ignoreCase = true) ||
                    it.contains("publishToLocalMaven", ignoreCase = true)
            }
    }
}
