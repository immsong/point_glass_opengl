#include "point_glass_opengl/point_glass_opengl_plugin.h"

#include <flutter/method_channel.h>
#include <flutter/plugin_registrar_windows.h>
#include <flutter/standard_method_codec.h>

#include <algorithm>
#include <cstdint>
#include <limits>
#include <memory>
#include <string>
#include <utility>

namespace
{
    constexpr uint32_t kDefaultTextureWidth = 400;
    constexpr uint32_t kDefaultTextureHeight = 400;
    constexpr uint64_t kMaxPixelBufferBytes = 512ull * 1024ull * 1024ull;

    int64_t GetInt64(const flutter::EncodableValue &value)
    {
        if (auto p = std::get_if<int64_t>(&value))
            return *p;
        if (auto p = std::get_if<int32_t>(&value))
            return *p;
        if (auto p = std::get_if<double>(&value))
            return static_cast<int64_t>(*p);
        return 0;
    }

    int64_t FindInt64(
        const flutter::EncodableMap *args,
        const std::string &key,
        int64_t fallback = 0)
    {
        if (!args)
            return fallback;

        const auto it = args->find(flutter::EncodableValue(key));
        if (it == args->end())
            return fallback;

        return GetInt64(it->second);
    }

    uint32_t FindUint32(
        const flutter::EncodableMap *args,
        const std::string &key,
        uint32_t fallback = 0)
    {
        const auto value = FindInt64(args, key, fallback);
        if (value <= 0)
            return fallback;

        return static_cast<uint32_t>(value);
    }

    uint32_t NormalizeSize(uint32_t value, uint32_t fallback)
    {
        return value > 0 ? value : fallback;
    }

    bool GetPixelBufferSize(uint32_t width, uint32_t height, size_t *out_size)
    {
        if (width == 0 || height == 0 || out_size == nullptr)
        {
            return false;
        }

        const uint64_t bytes =
            static_cast<uint64_t>(width) * static_cast<uint64_t>(height) * 4ull;

        if (bytes == 0 || bytes > kMaxPixelBufferBytes)
        {
            return false;
        }

        if (bytes > static_cast<uint64_t>((std::numeric_limits<size_t>::max)()))
        {
            return false;
        }

        *out_size = static_cast<size_t>(bytes);
        return true;
    }

    bool ResizeStateBuffer(
        const std::shared_ptr<point_glass_opengl::TextureState> &state,
        uint32_t width,
        uint32_t height)
    {
        if (!state)
            return false;

        size_t buffer_size = 0;
        if (!GetPixelBufferSize(width, height, &buffer_size))
        {
            return false;
        }

        std::lock_guard<std::mutex> lock(state->mutex);

        if (state->disposed)
        {
            return false;
        }

        state->width = width;
        state->height = height;
        state->pixels.resize(buffer_size);

        return true;
    }
} // namespace

namespace point_glass_opengl
{

    void PointGlassOpenglPlugin::RegisterWithRegistrar(
        flutter::PluginRegistrarWindows *registrar)
    {
        auto channel =
            std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
                registrar->messenger(),
                "point_glass_opengl",
                &flutter::StandardMethodCodec::GetInstance());

        auto plugin = std::make_unique<PointGlassOpenglPlugin>(registrar);

        channel->SetMethodCallHandler(
            [plugin_pointer = plugin.get()](const auto &call, auto result)
            {
                plugin_pointer->HandleMethodCall(call, std::move(result));
            });

        registrar->AddPlugin(std::move(plugin));
    }

    PointGlassOpenglPlugin::PointGlassOpenglPlugin(
        flutter::PluginRegistrarWindows *registrar)
        : registrar_(registrar) {}

    PointGlassOpenglPlugin::~PointGlassOpenglPlugin()
    {
        DisposeAllTextures();
    }

    void PointGlassOpenglPlugin::HandleMethodCall(
        const flutter::MethodCall<flutter::EncodableValue> &method_call,
        std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result)
    {
        const auto &method = method_call.method_name();

        if (method == "createTexture")
        {
            const auto *args =
                std::get_if<flutter::EncodableMap>(method_call.arguments());

            const auto renderer_ptr = FindInt64(args, "rendererPtr");
            const auto render_func_ptr = FindInt64(args, "renderFuncPtr");

            if (renderer_ptr == 0 || render_func_ptr == 0)
            {
                result->Error("invalid_args", "rendererPtr or renderFuncPtr is null");
                return;
            }

            const auto width = NormalizeSize(
                FindUint32(args, "width", kDefaultTextureWidth),
                kDefaultTextureWidth);
            const auto height = NormalizeSize(
                FindUint32(args, "height", kDefaultTextureHeight),
                kDefaultTextureHeight);

            auto state = std::make_shared<TextureState>();
            state->renderer = reinterpret_cast<void *>(renderer_ptr);
            state->render_func =
                reinterpret_cast<RenderToBufferFunc>(render_func_ptr);

            if (!ResizeStateBuffer(state, width, height))
            {
                result->Error("invalid_size", "Invalid texture size");
                return;
            }

            auto texture = std::make_shared<flutter::TextureVariant>(
                flutter::PixelBufferTexture(
                    [state](size_t, size_t) -> const FlutterDesktopPixelBuffer *
                    {
                        std::lock_guard<std::mutex> lock(state->mutex);

                        if (!state->disposed && state->render_func && state->renderer &&
                            !state->pixels.empty())
                        {
                            state->render_func(
                                state->renderer,
                                state->pixels.data(),
                                state->width,
                                state->height);
                        }

                        state->pixel_buffer.buffer =
                            state->pixels.empty() ? nullptr : state->pixels.data();
                        state->pixel_buffer.width = state->width;
                        state->pixel_buffer.height = state->height;

                        return &state->pixel_buffer;
                    }));

            const int64_t texture_id =
                registrar_->texture_registrar()->RegisterTexture(texture.get());

            if (texture_id < 0)
            {
                result->Error("texture_error", "Failed to register texture");
                return;
            }

            textures_[texture_id] = texture;
            texture_states_[texture_id] = state;

            result->Success(flutter::EncodableValue(texture_id));
            return;
        }

        if (method == "resizeTexture")
        {
            const auto *args =
                std::get_if<flutter::EncodableMap>(method_call.arguments());

            const auto texture_id = FindInt64(args, "textureId", -1);
            const auto width = FindUint32(args, "width");
            const auto height = FindUint32(args, "height");

            if (width == 0 || height == 0)
            {
                result->Error("invalid_size", "width and height must be greater than 0");
                return;
            }

            if (texture_id >= 0)
            {
                const auto it = texture_states_.find(texture_id);
                if (it == texture_states_.end())
                {
                    result->Error("not_found", "Texture not found");
                    return;
                }

                if (!ResizeStateBuffer(it->second, width, height))
                {
                    result->Error("invalid_size", "Invalid texture size");
                    return;
                }

                MarkTextureFrameAvailable(texture_id);
            }
            else
            {
                for (const auto &[id, state] : texture_states_)
                {
                    if (ResizeStateBuffer(state, width, height))
                    {
                        MarkTextureFrameAvailable(id);
                    }
                }
            }

            result->Success(flutter::EncodableValue(nullptr));
            return;
        }

        if (method == "requestRender")
        {
            const auto *args =
                std::get_if<flutter::EncodableMap>(method_call.arguments());

            const auto texture_id = FindInt64(args, "textureId", -1);

            if (texture_id >= 0)
            {
                if (texture_states_.find(texture_id) == texture_states_.end())
                {
                    result->Error("not_found", "Texture not found");
                    return;
                }

                MarkTextureFrameAvailable(texture_id);
            }
            else
            {
                MarkAllTextureFramesAvailable();
            }

            result->Success(flutter::EncodableValue(nullptr));
            return;
        }

        if (method == "disposeTexture")
        {
            const auto *args =
                std::get_if<flutter::EncodableMap>(method_call.arguments());

            const auto texture_id = FindInt64(args, "textureId", -1);

            if (texture_id < 0)
            {
                result->Error("invalid_args", "textureId is required");
                return;
            }

            DisposeTexture(texture_id);
            result->Success(flutter::EncodableValue(nullptr));
            return;
        }

        result->NotImplemented();
    }

    void PointGlassOpenglPlugin::MarkTextureFrameAvailable(int64_t texture_id)
    {
        registrar_->texture_registrar()->MarkTextureFrameAvailable(texture_id);
    }

    void PointGlassOpenglPlugin::MarkAllTextureFramesAvailable()
    {
        for (const auto &[texture_id, _] : texture_states_)
        {
            MarkTextureFrameAvailable(texture_id);
        }
    }

    void PointGlassOpenglPlugin::DisposeTexture(int64_t texture_id)
    {
        const auto texture_it = textures_.find(texture_id);
        const auto state_it = texture_states_.find(texture_id);

        if (texture_it == textures_.end())
        {
            return;
        }

        auto texture = texture_it->second;
        std::shared_ptr<TextureState> state;

        if (state_it != texture_states_.end())
        {
            state = state_it->second;

            std::lock_guard<std::mutex> lock(state->mutex);
            state->disposed = true;
            state->renderer = nullptr;
            state->render_func = nullptr;
        }

        textures_.erase(texture_it);

        if (state_it != texture_states_.end())
        {
            texture_states_.erase(state_it);
        }

        registrar_->texture_registrar()->UnregisterTexture(
            texture_id,
            [texture, state]()
            {
                // Maintain texture/state lifecycle
            });
    }

    void PointGlassOpenglPlugin::DisposeAllTextures()
    {
        std::vector<int64_t> texture_ids;
        texture_ids.reserve(textures_.size());

        for (const auto &[texture_id, _] : textures_)
        {
            texture_ids.push_back(texture_id);
        }

        for (const auto texture_id : texture_ids)
        {
            DisposeTexture(texture_id);
        }
    }

} // namespace point_glass_opengl

extern "C"
{
    __declspec(dllexport) void PointGlassOpenglPluginRegisterWithRegistrar(
        FlutterDesktopPluginRegistrarRef registrar)
    {
        point_glass_opengl::PointGlassOpenglPlugin::RegisterWithRegistrar(
            flutter::PluginRegistrarManager::GetInstance()
                ->GetRegistrar<flutter::PluginRegistrarWindows>(registrar));
    }
}