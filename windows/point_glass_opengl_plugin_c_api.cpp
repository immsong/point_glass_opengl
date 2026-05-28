#include "include/point_glass_opengl/point_glass_opengl_plugin_c_api.h"

#include <flutter/plugin_registrar_windows.h>

#include "point_glass_opengl_plugin.h"

void PointGlassOpenglPluginCApiRegisterWithRegistrar(
    FlutterDesktopPluginRegistrarRef registrar) {
  point_glass_opengl::PointGlassOpenglPlugin::RegisterWithRegistrar(
      flutter::PluginRegistrarManager::GetInstance()
          ->GetRegistrar<flutter::PluginRegistrarWindows>(registrar));
}
