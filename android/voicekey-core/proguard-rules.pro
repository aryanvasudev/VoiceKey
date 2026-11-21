# UniFFI and JNA Proguard preservation rules
-keep class com.voicekey.core.ffi.** { *; }
-keep class com.voicekey.core.** { *; }
-keep class com.sun.jna.** { *; }
-keepclassmembers class * extends com.sun.jna.Structure {
    <fields>;
}
-keepclassmembers class * implements com.sun.jna.Library {
    <methods>;
}
