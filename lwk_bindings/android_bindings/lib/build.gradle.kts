plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
    id("org.gradle.maven-publish")
    id("maven-publish")
}

android {
    namespace = "com.blockstream.lwk_bindings"
    compileSdk = 34

    defaultConfig {
        minSdk = 24

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        consumerProguardFiles("consumer-rules.pro")
    }

    buildTypes {
        release {
            isMinifyEnabled = false
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
    kotlinOptions {
        jvmTarget = "1.8"
    }

    lint {
        abortOnError = false
        checkReleaseBuilds =  false
    }
    publishing {
        singleVariant("release") {
            withSourcesJar()
            withJavadocJar()
        }
    }
}

dependencies {
    implementation("net.java.dev.jna:jna:5.13.0@aar")
    implementation ("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.7.3")


//    implementation("org.jetbrains.kotlin:kotlin-stdlib-jdk7")
//    implementation("androidx.appcompat:appcompat:1.6.1")
//    implementation("androidx.core:core-ktx:1.12.0")

    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.5")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.1")
}

val libraryVersion: String by project
publishing {
    repositories {
        maven {
            name = "lwkGitHubPackages"
            url = uri("https://maven.pkg.github.com/blockstream/lwk")
            credentials {
                username = System.getenv("GITHUB_ACTOR")
                password = System.getenv("GITHUB_TOKEN")
            }
        }
    }
    publications {
        create<MavenPublication>("maven") {
            groupId = "com.blockstream"
            artifactId = "lwk_bindings"
            version = libraryVersion

            afterEvaluate {
                from(components["release"])
            }

            pom {
                name.set("LWK")
                description.set("Liquid Wallet Kit for android.")
                url.set("https://blockstream.com")
                licenses {
                    license {
                        name.set("BSD-MIT")
                        url.set("https://github.com/blockstream/lwk/blob/main/LICENSE")
                    }
                }
                scm {
                    connection.set("scm:git:github.com/blockstream/lwk.git")
                    developerConnection.set("scm:git:ssh://github.com/blockstream/lwk.git")
                    url.set("https://github.com/blockstream/lwk")
                }
            }
        }
    }
}
