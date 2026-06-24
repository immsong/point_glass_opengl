#ifndef FLUTTER_PLUGIN_POINT_GLASS_OPENGL_PLUGIN_H_
#define FLUTTER_PLUGIN_POINT_GLASS_OPENGL_PLUGIN_H_

#include <flutter/method_channel.h>
#include <flutter/plugin_registrar_windows.h>
#include <flutter/standard_method_codec.h>
#include <flutter/texture_registrar.h>

#include <cstdint>
#include <map>
#include <memory>
#include <mutex>
#include <vector>

namespace point_glass_opengl
{
  using RenderToBufferFunc = void (*)(
      void *renderer,
      uint8_t *buffer,
      uint32_t width,
      uint32_t height);

  struct TextureState
  {
    void *renderer = nullptr;
    RenderToBufferFunc render_func = nullptr;

    uint32_t width = 1;
    uint32_t height = 1;

    std::vector<uint8_t> pixels;
    FlutterDesktopPixelBuffer pixel_buffer{};

    std::mutex mutex;
    bool disposed = false;
  };

  class PointGlassOpenglPlugin : public flutter::Plugin
  {
  public:
    static void RegisterWithRegistrar(flutter::PluginRegistrarWindows *registrar);

    explicit PointGlassOpenglPlugin(flutter::PluginRegistrarWindows *registrar);
    ~PointGlassOpenglPlugin() override;

    PointGlassOpenglPlugin(const PointGlassOpenglPlugin &) = delete;
    PointGlassOpenglPlugin &operator=(const PointGlassOpenglPlugin &) = delete;

  private:
    void HandleMethodCall(
        const flutter::MethodCall<flutter::EncodableValue> &method_call,
        std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result);

    void MarkTextureFrameAvailable(int64_t texture_id);
    void MarkAllTextureFramesAvailable();

    void DisposeTexture(int64_t texture_id);
    void DisposeAllTextures();

    flutter::PluginRegistrarWindows *registrar_;

    std::map<int64_t, std::shared_ptr<flutter::TextureVariant>> textures_;
    std::map<int64_t, std::shared_ptr<TextureState>> texture_states_;
  };
} // namespace point_glass_opengl

extern "C"
{
  __declspec(dllexport) void PointGlassOpenglPluginRegisterWithRegistrar(
      FlutterDesktopPluginRegistrarRef registrar);
}

#endif // FLUTTER_PLUGIN_POINT_GLASS_OPENGL_PLUGIN_H_