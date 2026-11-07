cmake_minimum_required(VERSION 3.25)
project(wii_menu_c_host C)

if(NOT DEFINED WII_MENU_CORE_INCLUDE_DIR OR NOT DEFINED WII_MENU_CORE_LIBRARY OR NOT DEFINED WII_MENU_CORE_DLL)
  message(FATAL_ERROR "Set WII_MENU_CORE_INCLUDE_DIR, WII_MENU_CORE_LIBRARY, and WII_MENU_CORE_DLL")
endif()

add_executable(wii-menu-c-host menu_host.c)
target_include_directories(wii-menu-c-host PRIVATE "${WII_MENU_CORE_INCLUDE_DIR}")
target_compile_definitions(wii-menu-c-host PRIVATE WII_MENU_CORE_SHARED=1)
target_link_libraries(wii-menu-c-host PRIVATE "${WII_MENU_CORE_LIBRARY}")
add_custom_command(TARGET wii-menu-c-host POST_BUILD
  COMMAND ${CMAKE_COMMAND} -E copy_if_different "${WII_MENU_CORE_DLL}" "$<TARGET_FILE_DIR:wii-menu-c-host>"
  VERBATIM)
