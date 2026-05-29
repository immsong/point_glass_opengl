#ifndef FLUTTER_PLUGIN_POINT_GLASS_OPENGL_PLUGIN_H_
#define FLUTTER_PLUGIN_POINT_GLASS_OPENGL_PLUGIN_H_

#include <flutter/method_channel.h>
#include <flutter/plugin_registrar_windows.h>

#include <memory>

namespace point_glass_opengl {

class PointGlassOpenglPlugin : public flutter::Plugin {
 public:
  static void RegisterWithRegistrar(flutter::PluginRegistrarWindows *registrar);

  PointGlassOpenglPlugin();

  virtual ~PointGlassOpenglPlugin();

  // Disallow copy and assign.
  PointGlassOpenglPlugin(const PointGlassOpenglPlugin&) = delete;
  PointGlassOpenglPlugin& operator=(const PointGlassOpenglPlugin&) = delete;

  // Called when a method is called on this plugin's channel from Dart.
  void HandleMethodCall(
      const flutter::MethodCall<flutter::EncodableValue> &method_call,
      std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result);
};

}  // namespace point_glass_opengl

#endif  // FLUTTER_PLUGIN_POINT_GLASS_OPENGL_PLUGIN_H_
