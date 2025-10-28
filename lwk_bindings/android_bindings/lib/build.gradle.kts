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

mavenPublishing {
    coordinates(groupId = "com.blockstream", artifactId = "lwk", version = libraryVersion)

    pom {
        name = "LWK"
        description = "Liquid Wallet Kit"
        url = "https://blockstream.com"
        licenses {
            license {
                name = "BSD-MIT"
                url = "https://github.com/blockstream/lwk/blob/main/LICENSE"
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

    publishToMavenCentral()
    signAllPublications()
}

extensions.configure<SigningExtension> {
    useGpgCmd()
}
