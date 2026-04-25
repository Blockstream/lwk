# Consumer ProGuard/R8 rules — applied to apps that consume this library.
# These rules are shipped inside the AAR (proguard.txt) and merged into
# the consuming app's R8 configuration when minification is enabled.

# JNA loads classes reflectively, so they must be kept.
-dontwarn java.awt.*
-keep class com.sun.jna.* { *; }
-keep class lwk.** { *; }
-keepclassmembers class * extends com.sun.jna.* { public *; }
-keepclassmembers class * extends lwk.** { public *; }
-dontwarn java.lang.invoke.StringConcatFactory