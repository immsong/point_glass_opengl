#include "include/point_glass_opengl/point_glass_opengl_plugin.h"

#include <flutter_linux/flutter_linux.h>
#include <gtk/gtk.h>
#include <sys/utsname.h>
#include <cstring>
#include <iostream>
#include <epoxy/gl.h>

#define POINT_GLASS_OPENG_PLUGIN(obj)                                      \
  (G_TYPE_CHECK_INSTANCE_CAST((obj), point_glass_opengl_plugin_get_type(), \
                              PointGlassOpenglPlugin))

// --- 1. 커스텀 OpenGL Texture 클래스 정의 (FlTextureGL 상속) ---
G_DECLARE_FINAL_TYPE(PointGlassTexture, point_glass_texture, POINT_GLASS, TEXTURE, FlTextureGL)

struct _PointGlassTexture
{
  FlTextureGL parent_instance;
  // TODO: 이후 여기에 Rust Renderer 포인터를 저장하여 그리기 명령을 내릴 예정입니다.
};

G_DEFINE_TYPE(PointGlassTexture, point_glass_texture, fl_texture_gl_get_type())

static void point_glass_texture_init(PointGlassTexture *self) {}

// Flutter 엔진이 "이 텍스처에 그릴 화면(OpenGL)을 내놔라!" 할 때마다 호출되는 콜백
// (이 콜백이 불릴 때는 이미 Flutter 렌더링 스레드의 OpenGL Context가 활성화된 상태입니다)
static gboolean point_glass_texture_populate(FlTextureGL *texture,
                                             uint32_t *target,
                                             uint32_t *name,
                                             uint32_t *width,
                                             uint32_t *height,
                                             GError **error)
{
  // 텍스처는 한 번만 생성해서 계속 재사용합니다.
  static uint32_t gl_tex_id = 0;

  if (gl_tex_id == 0)
  {
    g_print("[C++] Generating real OpenGL texture...\n");

    glGenTextures(1, &gl_tex_id);
    glBindTexture(GL_TEXTURE_2D, gl_tex_id);

    // 1x1 픽셀짜리 빨간색(Red) 데이터 (R, G, B, A)
    const uint8_t pixels[4] = {255, 0, 0, 255};

    // 그래픽 카드(VRAM)로 픽셀 데이터 전송
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, 1, 1, 0, GL_RGBA, GL_UNSIGNED_BYTE, pixels);

    // 텍스처 필터링 설정 (설정하지 않으면 검은 화면이 나올 수 있습니다)
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
  }

  // Flutter 엔진에게 우리가 만든 텍스처 정보를 넘겨줍니다.
  *target = GL_TEXTURE_2D;
  *name = gl_tex_id;

  // 원본 텍스처 크기를 1x1로 줍니다.
  // (Flutter UI의 400x400 위젯 크기에 맞춰서 그래픽카드가 쫙 늘려줍니다)
  *width = 1;
  *height = 1;

  return TRUE;
}

static void point_glass_texture_class_init(PointGlassTextureClass *klass)
{
  FlTextureGLClass *gl_class = FL_TEXTURE_GL_CLASS(klass);
  gl_class->populate = point_glass_texture_populate;
}

static PointGlassTexture *point_glass_texture_new()
{
  return POINT_GLASS_TEXTURE(g_object_new(point_glass_texture_get_type(), nullptr));
}
// ----------------------------------------------------------------

struct _PointGlassOpenglPlugin
{
  GObject parent_instance;
  FlPluginRegistrar *registrar; // Texture 등록을 위해 registrar 보관
};

G_DEFINE_TYPE(PointGlassOpenglPlugin, point_glass_opengl_plugin, g_object_get_type())

static void point_glass_opengl_plugin_handle_method_call(
    PointGlassOpenglPlugin *self,
    FlMethodCall *method_call)
{
  g_autoptr(FlMethodResponse) response = nullptr;
  const gchar *method = fl_method_call_get_name(method_call);

  if (strcmp(method, "createTexture") == 0)
  {
    g_print("[C++] createTexture called from Dart (Linux)!\n");

    // 2. 텍스처 생성 및 등록
    FlTextureRegistrar *texture_registrar = fl_plugin_registrar_get_texture_registrar(self->registrar);
    PointGlassTexture *texture = point_glass_texture_new();

    fl_texture_registrar_register_texture(texture_registrar, FL_TEXTURE(texture));

    // 3. 엔진으로부터 발급받은 "진짜 텍스처 ID"를 Dart로 반환
    int64_t texture_id = fl_texture_get_id(FL_TEXTURE(texture));
    g_print("[C++] Real Texture ID generated: %ld\n", texture_id);

    g_autoptr(FlValue) result = fl_value_new_int(texture_id);
    response = FL_METHOD_RESPONSE(fl_method_success_response_new(result));
  }
  else
  {
    response = FL_METHOD_RESPONSE(fl_method_not_implemented_response_new());
  }

  fl_method_call_respond(method_call, response, nullptr);
}

static void point_glass_opengl_plugin_dispose(GObject *object)
{
  G_OBJECT_CLASS(point_glass_opengl_plugin_parent_class)->dispose(object);
}

static void point_glass_opengl_plugin_class_init(PointGlassOpenglPluginClass *klass)
{
  G_OBJECT_CLASS(klass)->dispose = point_glass_opengl_plugin_dispose;
}

static void point_glass_opengl_plugin_init(PointGlassOpenglPlugin *self) {}

static void method_call_cb(FlMethodChannel *channel, FlMethodCall *method_call,
                           gpointer user_data)
{
  PointGlassOpenglPlugin *plugin = POINT_GLASS_OPENG_PLUGIN(user_data);
  point_glass_opengl_plugin_handle_method_call(plugin, method_call);
}

void point_glass_opengl_plugin_register_with_registrar(FlPluginRegistrar *registrar)
{
  PointGlassOpenglPlugin *plugin = POINT_GLASS_OPENG_PLUGIN(
      g_object_new(point_glass_opengl_plugin_get_type(), nullptr));

  plugin->registrar = FL_PLUGIN_REGISTRAR(g_object_ref(registrar));

  g_autoptr(FlStandardMethodCodec) codec = fl_standard_method_codec_new();
  g_autoptr(FlMethodChannel) channel =
      fl_method_channel_new(fl_plugin_registrar_get_messenger(registrar),
                            "point_glass_opengl",
                            FL_METHOD_CODEC(codec));
  fl_method_channel_set_method_call_handler(channel, method_call_cb,
                                            g_object_ref(plugin),
                                            g_object_unref);
  g_object_unref(plugin);
}