find_package(Git QUIET)
set(GIT_REV "1.1-unofficial")
if(GIT_FOUND)
    execute_process(COMMAND "${GIT_EXECUTABLE}" describe --tags --always --dirty
        WORKING_DIRECTORY "${SOURCE_DIR}"
        OUTPUT_VARIABLE DESCRIPTION
        RESULT_VARIABLE RESULT
        ERROR_QUIET OUTPUT_STRIP_TRAILING_WHITESPACE)
    if(RESULT EQUAL 0 AND NOT DESCRIPTION STREQUAL "")
        set(GIT_REV "${DESCRIPTION}")
    endif()
endif()
configure_file("${CMAKE_CURRENT_LIST_DIR}/spit-version.h.in" "${OUTPUT_FILE}" @ONLY)
