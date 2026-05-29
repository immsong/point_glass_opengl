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
  void *renderer_ptr;          // Rust Renderer 인스턴스 주소
  void (*render_func)(void *); // Rust render_frame 함수 주소
  uint32_t tex_id;             // 엔진에 넘길 텍스처 ID
  uint32_t fbo_id;             // Rust가 그릴 캔버스(FBO) ID
};

G_DEFINE_TYPE(PointGlassTexture, point_glass_texture, fl_texture_gl_get_type())

static void point_glass_texture_init(PointGlassTexture *self) {}

// Flutter 엔진이 "이 텍스처에 그릴 화면(OpenGL)을 내놔라!" 할 때마다 호출되는 콜백
// (이 콜백이 불릴 때는 이미 Flutter 렌더링 스레드의 OpenGL Context가 활성화된 상태입니다)
static gboolean point_glass_texture_populate(FlTextureGL *texture,
                                             uint32_t *target, uint32_t *name,
                                             uint32_t *width, uint32_t *height,
                                             GError **error)
{
  PointGlassTexture *pg_texture = POINT_GLASS_TEXTURE(texture);

  // 1. 최초 1회 텍스처와 FBO 생성
  if (pg_texture->tex_id == 0)
  {
    g_print("[C++] Generating OpenGL Texture and FBO for Rust...\n");

    glGenTextures(1, &pg_texture->tex_id);
    glBindTexture(GL_TEXTURE_2D, pg_texture->tex_id);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, 400, 400, 0, GL_RGBA, GL_UNSIGNED_BYTE, nullptr);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);

    // Rust가 텍스처에 직접 그릴 수 있도록 FBO를 묶어줍니다.
    glGenFramebuffers(1, &pg_texture->fbo_id);
    glBindFramebuffer(GL_FRAMEBUFFER, pg_texture->fbo_id);
    glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, pg_texture->tex_id, 0);
    glBindFramebuffer(GL_FRAMEBUFFER, 0);
  }

  // 2. FBO 바인딩 후 Rust 렌더러 호출!
  if (pg_texture->render_func && pg_texture->renderer_ptr)
  {
    GLint previous_fbo;
    glGetIntegerv(GL_FRAMEBUFFER_BINDING, &previous_fbo);

    glBindFramebuffer(GL_FRAMEBUFFER, pg_texture->fbo_id);
    glViewport(0, 0, 400, 400);

    // 🚀 드디어 Rust가 화면에 그림을 그립니다! 🚀
    pg_texture->render_func(pg_texture->renderer_ptr);

    glBindFramebuffer(GL_FRAMEBUFFER, previous_fbo);
  }

  // 3. 엔진에 결과 텍스처 보고
  *target = GL_TEXTURE_2D;
  *name = pg_texture->tex_id;
  *width = 400;
  *height = 400;

  return TRUE;
}

static void point_glass_texture_class_init(PointGlassTextureClass *klass)
{
  FlTextureGLClass *gl_class = FL_TEXTURE_GL_CLASS(klass);
  gl_class->populate = point_glass_texture_populate;
}

// static PointGlassTexture *point_glass_texture_new()
// {
//   return POINT_GLASS_TEXTURE(g_object_new(point_glass_texture_get_type(), nullptr));
// }
// ----------------------------------------------------------------

struct _PointGlassOpenglPlugin
{
  GObject parent_instance;
  FlPluginRegistrar *registrar; // Texture 등록을 위해 registrar 보관
};

G_DEFINE_TYPE(PointGlassOpenglPlugin, point_glass_opengl_plugin, g_object_get_type())

static void point_glass_opengl_plugin_handle_method_call(
    PointGlassOpenglPlugin *self, FlMethodCall *method_call)
{
  g_autoptr(FlMethodResponse) response = nullptr;
  const gchar *method = fl_method_call_get_name(method_call);

  if (strcmp(method, "createTexture") == 0)
  {
    // Dart가 넘겨준 메모리 주소 2개(Renderer, RenderFunction)를 파싱
    FlValue *args = fl_method_call_get_args(method_call);
    int64_t renderer_address = fl_value_get_int(fl_value_lookup_string(args, "rendererPtr"));
    int64_t render_func_address = fl_value_get_int(fl_value_lookup_string(args, "renderFuncPtr"));

    FlTextureRegistrar *texture_registrar = fl_plugin_registrar_get_texture_registrar(self->registrar);
    PointGlassTexture *texture = POINT_GLASS_TEXTURE(g_object_new(point_glass_texture_get_type(), nullptr));

    // 텍스처 객체에 주소 저장 및 초기화
    texture->renderer_ptr = reinterpret_cast<void *>(renderer_address);
    texture->render_func = reinterpret_cast<void (*)(void *)>(render_func_address);
    texture->tex_id = 0;
    texture->fbo_id = 0;

    fl_texture_registrar_register_texture(texture_registrar, FL_TEXTURE(texture));

    int64_t texture_id = fl_texture_get_id(FL_TEXTURE(texture));
    g_print("[C++] Texture pipeline established. ID: %ld\n", texture_id);

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