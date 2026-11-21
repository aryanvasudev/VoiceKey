# ProGuard rules for VoiceKey App & IME
-keep class com.voicekey.app.ime.** { *; }
-keep class com.voicekey.app.service.** { *; }
-keep class com.voicekey.core.** { *; }
-keep class com.voicekey.core.ffi.** { *; }
-keep class com.sun.jna.** { *; }
-keepclassmembers class * extends com.sun.jna.Structure {
    <fields>;
}
