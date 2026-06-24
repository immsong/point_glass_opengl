cmake_minimum_required(VERSION 3.14)

# Project-level configuration.
set(PROJECT_NAME "point_glass_opengl")
project(${PROJECT_NAME} LANGUAGES CXX)

set(PLUGIN_NAME "point_glass_opengl_plugin")

list(APPEND PLUGIN_SOURCES
  "point_glass_opengl_plugin.cpp"
)

add_library(${PLUGIN_NAME} SHARED
  ${PLUGIN_SOURCES}
)

apply_standard_settings(${PLUGIN_NAME})

set_target_properties(${PLUGIN_NAME} PROPERTIES
  CXX_VISIBILITY_PRESET hidden)
target_compile_definitions(${PLUGIN_NAME} PRIVATE FLUTTER_PLUGIN_IMPL)

target_include_directories(${PLUGIN_NAME} INTERFACE
  "${CMAKE_CURRENT_SOURCE_DIR}/include")

target_include_directories(${PLUGIN_NAME} PRIVATE
  "${CMAKE_CURRENT_SOURCE_DIR}/include")

target_link_libraries(${PLUGIN_NAME} PRIVATE flutter flutter_wrapper_plugin)

set(point_glass_opengl_bundled_libraries
  $<TARGET_FILE:${PLUGIN_NAME}>
  "${CMAKE_CURRENT_SOURCE_DIR}/shared/point_glass_opengl_core.dll"
  PARENT_SCOPE
)