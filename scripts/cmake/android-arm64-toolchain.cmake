# VoiceKey CMake Toolchain wrapper for Android arm64-v8a
set(ANDROID_ABI "arm64-v8a" CACHE STRING "Target ABI" FORCE)
set(ANDROID_PLATFORM "android-29" CACHE STRING "Target Android API level" FORCE)
set(CMAKE_SYSTEM_NAME "Android" CACHE STRING "" FORCE)
set(CMAKE_SYSTEM_VERSION "29" CACHE STRING "" FORCE)
set(CMAKE_ANDROID_ARCH_ABI "arm64-v8a" CACHE STRING "" FORCE)

if(DEFINED ENV{ANDROID_NDK_HOME})
    set(_VK_NDK "$ENV{ANDROID_NDK_HOME}")
elseif(DEFINED ENV{ANDROID_NDK})
    set(_VK_NDK "$ENV{ANDROID_NDK}")
else()
    message(FATAL_ERROR "Neither ANDROID_NDK_HOME nor ANDROID_NDK is set in environment.")
endif()

include("${_VK_NDK}/build/cmake/android.toolchain.cmake")
