cmake_minimum_required(VERSION 3.20)
if(CMAKE_SOURCE_DIR STREQUAL CMAKE_BINARY_DIR)
  message(FATAL_ERROR "Use an out-of-source build: cmake -S . -B build (or make)")
endif()
file(STRINGS "${CMAKE_CURRENT_SOURCE_DIR}/VERSION" STUTOOLS_VERSION LIMIT_COUNT 1)
project(stutools VERSION ${STUTOOLS_VERSION} LANGUAGES C)
if(CMAKE_SYSTEM_NAME STREQUAL "Linux" AND CMAKE_INSTALL_PREFIX_INITIALIZED_TO_DEFAULT)
  set(CMAKE_INSTALL_PREFIX "/usr" CACHE PATH "Installation prefix" FORCE)
endif()
set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
include(GNUInstallDirs)
option(STUTOOLS_NATIVE_BACKEND "Build the portable synchronous file backend" OFF)
if(WIN32 OR APPLE OR CMAKE_SYSTEM_NAME STREQUAL "FreeBSD" OR STUTOOLS_NATIVE_BACKEND)
  enable_testing()
  add_subdirectory(platform)
  return()
endif()
if(NOT CMAKE_SYSTEM_NAME STREQUAL "Linux")
  message(FATAL_ERROR "Unsupported OS; use -DSTUTOOLS_NATIVE_BACKEND=ON for the portable file backend")
endif()
set(STUTOOLS_SYSTEMD_UNIT_DIR "lib/systemd/system" CACHE PATH "systemd unit installation directory")
foreach(header libaio.h pci/pci.h numa.h readline/readline.h uuid/uuid.h)
  string(MAKE_C_IDENTIFIER "${header}" dep)
  find_path(${dep}_INCLUDE_DIR NAMES ${header} REQUIRED)
endforeach()
foreach(lib aio pci numa readline uuid)
  find_library(${lib}_LIBRARY NAMES ${lib} REQUIRED)
endforeach()

include(${CMAKE_CURRENT_SOURCE_DIR}/utils/Version.cmake)

enable_testing()

add_subdirectory(iotests)
add_subdirectory(spit)
add_subdirectory(utils)
add_subdirectory(report)




find_package(Python3 COMPONENTS Interpreter QUIET)
if(Python3_Interpreter_FOUND)
  add_test(NAME spitBuildVersion COMMAND ${Python3_EXECUTABLE}
    ${CMAKE_CURRENT_SOURCE_DIR}/tests/test_build_version.py)
  add_test(NAME spitSafety COMMAND ${Python3_EXECUTABLE}
    ${CMAKE_CURRENT_SOURCE_DIR}/tests/test_spit_safety.py $<TARGET_FILE:spit>)
endif()

if(Python3_Interpreter_FOUND)
  set_tests_properties(spitBuildVersion spitSafety PROPERTIES LABELS smoke)
endif()
