cmake_minimum_required(VERSION 3.24)
project(eurochef_fbx_exporter LANGUAGES CXX)

if(NOT WIN32)
    message(FATAL_ERROR "The current EuroChef FBX helper is supported on Windows x64 only")
endif()

if(NOT DEFINED FBX_SDK_ROOT OR FBX_SDK_ROOT STREQUAL "")
    if(DEFINED ENV{FBX_SDK_ROOT} AND NOT "$ENV{FBX_SDK_ROOT}" STREQUAL "")
        set(FBX_SDK_ROOT "$ENV{FBX_SDK_ROOT}")
    else()
        message(FATAL_ERROR "FBX_SDK_ROOT is required and must point to an external Autodesk FBX SDK 2020.x installation")
    endif()
endif()

file(TO_CMAKE_PATH "${FBX_SDK_ROOT}" FBX_SDK_ROOT)
set(FBX_INCLUDE_DIR "${FBX_SDK_ROOT}/include")
if(NOT EXISTS "${FBX_INCLUDE_DIR}/fbxsdk.h")
    message(FATAL_ERROR "fbxsdk.h was not found under ${FBX_INCLUDE_DIR}")
endif()

file(GLOB_RECURSE FBX_LIBRARY_CANDIDATES CONFIGURE_DEPENDS
    "${FBX_SDK_ROOT}/lib/*/x64/release/libfbxsdk-md.lib"
    "${FBX_SDK_ROOT}/lib/x64/release/libfbxsdk-md.lib")
list(SORT FBX_LIBRARY_CANDIDATES)
list(LENGTH FBX_LIBRARY_CANDIDATES FBX_LIBRARY_COUNT)
if(FBX_LIBRARY_COUNT EQUAL 0)
    message(FATAL_ERROR "No Windows x64 release static FBX SDK library libfbxsdk-md.lib was found below ${FBX_SDK_ROOT}/lib")
endif()
list(GET FBX_LIBRARY_CANDIDATES 0 FBX_LIBRARY)

add_executable(fbx_export_helper src/main.cpp)
target_compile_features(fbx_export_helper PRIVATE cxx_std_17)
target_include_directories(fbx_export_helper PRIVATE "${FBX_INCLUDE_DIR}")
target_link_libraries(fbx_export_helper PRIVATE "${FBX_LIBRARY}")

file(GLOB_RECURSE FBX_OPTIONAL_LIBRARIES CONFIGURE_DEPENDS
    "${FBX_SDK_ROOT}/lib/*/x64/release/libxml2-md.lib"
    "${FBX_SDK_ROOT}/lib/*/x64/release/zlib-md.lib"
    "${FBX_SDK_ROOT}/lib/*/x64/release/libxml2.lib"
    "${FBX_SDK_ROOT}/lib/*/x64/release/zlib.lib"
    "${FBX_SDK_ROOT}/lib/x64/release/libxml2-md.lib"
    "${FBX_SDK_ROOT}/lib/x64/release/zlib-md.lib"
    "${FBX_SDK_ROOT}/lib/x64/release/libxml2.lib"
    "${FBX_SDK_ROOT}/lib/x64/release/zlib.lib")
if(FBX_OPTIONAL_LIBRARIES)
    target_link_libraries(fbx_export_helper PRIVATE ${FBX_OPTIONAL_LIBRARIES} bcrypt.lib)
else()
    message(FATAL_ERROR "Static FBX SDK requires Windows x64 release libxml2 and zlib libraries, but they were not found below ${FBX_SDK_ROOT}/lib")
endif()

if(MSVC)
    target_compile_options(fbx_export_helper PRIVATE /W4 /permissive- /utf-8 /EHsc)
    target_compile_definitions(fbx_export_helper PRIVATE _CRT_SECURE_NO_WARNINGS)
endif()

message(STATUS "FBX SDK root: ${FBX_SDK_ROOT}")
message(STATUS "FBX SDK library: ${FBX_LIBRARY}")
