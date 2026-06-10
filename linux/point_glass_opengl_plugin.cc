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

// 커스텀 OpenGL Texture 클래스 정의 (FlTextureGL 상속)
// Flutter 프레임워크가 렌더링 스레드에서 접근할 수 있는 텍스처 메모리를 관리합니다.
G_DECLARE_FINAL_TYPE(PointGlassTexture, point_glass_texture, POINT_GLASS, TEXTURE, FlTextureGL)

struct _PointGlassTexture
{
  FlTextureGL parent_instance;
  void *renderer_ptr;
  void (*render_func)(void *);

  // OpenGL 메모리 식별자
  uint32_t tex_id;
  uint32_t fbo_id;
  uint32_t depth_rbo_id; // 3D 원근감을 위한 깊이 버퍼(Depth Renderbuffer)

  // 해상도 관리 (Flutter UI 스레드와 렌더링 스레드 간의 충돌 방지)
  uint32_t current_width;
  uint32_t current_height;
  gint desired_width;
  gint desired_height;
};

G_DEFINE_TYPE(PointGlassTexture, point_glass_texture, fl_texture_gl_get_type())

static void point_glass_texture_init(PointGlassTexture *self) {}

// Flutter 엔진이 화면을 새로 그릴 때마다 렌더링 스레드에서 호출하는 콜백 함수입니다.
static gboolean point_glass_texture_populate(FlTextureGL *texture,
                                             uint32_t *target, uint32_t *name,
                                             uint32_t *width, uint32_t *height,
                                             GError **error)
{
  PointGlassTexture *pg_texture = POINT_GLASS_TEXTURE(texture);

  // UI 스레드에서 변경된 해상도를 원자성(Atomic) 읽기를 통해 스레드 안전하게 가져옵니다.
  uint32_t dw = (uint32_t)g_atomic_int_get(&pg_texture->desired_width);
  uint32_t dh = (uint32_t)g_atomic_int_get(&pg_texture->desired_height);
  if (dw == 0)
    dw = 1;
  if (dh == 0)
    dh = 1;

  // 창 크기 변경 감지 시 기존 OpenGL 리소스를 메모리 누수 없이 삭제합니다.
  if (pg_texture->tex_id != 0 &&
      (pg_texture->current_width != dw || pg_texture->current_height != dh))
  {
    glDeleteFramebuffers(1, &pg_texture->fbo_id);
    glDeleteRenderbuffers(1, &pg_texture->depth_rbo_id);
    glDeleteTextures(1, &pg_texture->tex_id);
    pg_texture->tex_id = 0;
    pg_texture->fbo_id = 0;
    pg_texture->depth_rbo_id = 0;
  }

  // 최초 생성 또는 해상도 변경 시 새로운 FBO 및 버퍼를 할당합니다.
  if (pg_texture->tex_id == 0)
  {
    glGenTextures(1, &pg_texture->tex_id);
    glBindTexture(GL_TEXTURE_2D, pg_texture->tex_id);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, dw, dh, 0, GL_RGBA, GL_UNSIGNED_BYTE, nullptr);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);

    // 3D 렌더링의 올바른 앞뒤 순서를 판단하기 위한 깊이 버퍼를 생성합니다.
    glGenRenderbuffers(1, &pg_texture->depth_rbo_id);
    glBindRenderbuffer(GL_RENDERBUFFER, pg_texture->depth_rbo_id);
    glRenderbufferStorage(GL_RENDERBUFFER, GL_DEPTH_COMPONENT24, dw, dh);

    glGenFramebuffers(1, &pg_texture->fbo_id);
    glBindFramebuffer(GL_FRAMEBUFFER, pg_texture->fbo_id);
    glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, pg_texture->tex_id, 0);
    glFramebufferRenderbuffer(GL_FRAMEBUFFER, GL_DEPTH_ATTACHMENT, GL_RENDERBUFFER, pg_texture->depth_rbo_id);

    glBindFramebuffer(GL_FRAMEBUFFER, 0);

    pg_texture->current_width = dw;
    pg_texture->current_height = dh;
  }

  // FBO를 바인딩하고 Rust의 렌더링 함수에 제어권을 넘깁니다.
  if (pg_texture->render_func && pg_texture->renderer_ptr)
  {
    GLint previous_fbo;
    glGetIntegerv(GL_FRAMEBUFFER_BINDING, &previous_fbo);

    glBindFramebuffer(GL_FRAMEBUFFER, pg_texture->fbo_id);
    glViewport(0, 0, dw, dh);

    // Rust가 화면에 그림을 그립니다.
    pg_texture->render_func(pg_texture->renderer_ptr);

    glBindFramebuffer(GL_FRAMEBUFFER, previous_fbo);
  }

  // 그려진 텍스처의 식별자를 Flutter 엔진에 전달합니다.
  *target = GL_TEXTURE_2D;
  *name = pg_texture->tex_id;
  *width = dw;
  *height = dh;

  return TRUE;
}

static void point_glass_texture_class_init(PointGlassTextureClass *klass)
{
  FlTextureGLClass *gl_class = FL_TEXTURE_GL_CLASS(klass);
  gl_class->populate = point_glass_texture_populate;
}

// ----------------------------------------------------------------
// Flutter Method Channel 핸들러 (플러그인 주 진입점)
// ----------------------------------------------------------------
struct _PointGlassOpenglPlugin
{
  GObject parent_instance;
  FlPluginRegistrar *registrar;
  PointGlassTexture *current_texture;
};

G_DEFINE_TYPE(PointGlassOpenglPlugin, point_glass_opengl_plugin, g_object_get_type())

static void point_glass_opengl_plugin_handle_method_call(
    PointGlassOpenglPlugin *self, FlMethodCall *method_call)
{
  g_autoptr(FlMethodResponse) response = nullptr;
  const gchar *method = fl_method_call_get_name(method_call);

  if (strcmp(method, "createTexture") == 0)
  {
    // Dart가 전달한 Rust 구조체 포인터와 초기 해상도를 파싱합니다.
    FlValue *args = fl_method_call_get_args(method_call);
    int64_t renderer_address = fl_value_get_int(fl_value_lookup_string(args, "rendererPtr"));
    int64_t render_func_address = fl_value_get_int(fl_value_lookup_string(args, "renderFuncPtr"));
    int64_t init_width = fl_value_get_int(fl_value_lookup_string(args, "width"));
    int64_t init_height = fl_value_get_int(fl_value_lookup_string(args, "height"));

    FlTextureRegistrar *texture_registrar = fl_plugin_registrar_get_texture_registrar(self->registrar);
    PointGlassTexture *texture = POINT_GLASS_TEXTURE(g_object_new(point_glass_texture_get_type(), nullptr));

    self->current_texture = texture;
    texture->renderer_ptr = reinterpret_cast<void *>(renderer_address);
    texture->render_func = reinterpret_cast<void (*)(void *)>(render_func_address);
    texture->tex_id = 0;
    texture->fbo_id = 0;
    texture->depth_rbo_id = 0;
    texture->current_width = 0;
    texture->current_height = 0;
    g_atomic_int_set(&texture->desired_width, (gint)init_width);
    g_atomic_int_set(&texture->desired_height, (gint)init_height);

    fl_texture_registrar_register_texture(texture_registrar, FL_TEXTURE(texture));
    int64_t texture_id = fl_texture_get_id(FL_TEXTURE(texture));

    g_autoptr(FlValue) result = fl_value_new_int(texture_id);
    response = FL_METHOD_RESPONSE(fl_method_success_response_new(result));
  }
  else if (strcmp(method, "requestRender") == 0)
  {
    // Dart의 화면 갱신 요청을 받아 렌더링 큐에 추가합니다.
    if (self->current_texture)
    {
      FlTextureRegistrar *texture_registrar = fl_plugin_registrar_get_texture_registrar(self->registrar);
      fl_texture_registrar_mark_texture_frame_available(texture_registrar, FL_TEXTURE(self->current_texture));
    }
    response = FL_METHOD_RESPONSE(fl_method_success_response_new(nullptr));
  }
  else if (strcmp(method, "resizeTexture") == 0)
  {
    // 스레드 안전성을 보장하기 위해 g_atomic_int_set을 사용하여 해상도 변경을 등록합니다.
    FlValue *args = fl_method_call_get_args(method_call);
    if (self->current_texture)
    {
      gint w = (gint)fl_value_get_int(fl_value_lookup_string(args, "width"));
      gint h = (gint)fl_value_get_int(fl_value_lookup_string(args, "height"));
      g_atomic_int_set(&self->current_texture->desired_width, w);
      g_atomic_int_set(&self->current_texture->desired_height, h);
    }
    response = FL_METHOD_RESPONSE(fl_method_success_response_new(nullptr));
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

static void method_call_cb(FlMethodChannel *channel, FlMethodCall *method_call, gpointer user_data)
{
  PointGlassOpenglPlugin *plugin = POINT_GLASS_OPENG_PLUGIN(user_data);
  point_glass_opengl_plugin_handle_method_call(plugin, method_call);
}

void point_glass_opengl_plugin_register_with_registrar(FlPluginRegistrar *registrar)
{
  PointGlassOpenglPlugin *plugin = POINT_GLASS_OPENG_PLUGIN(g_object_new(point_glass_opengl_plugin_get_type(), nullptr));
  plugin->registrar = FL_PLUGIN_REGISTRAR(g_object_ref(registrar));
  g_autoptr(FlStandardMethodCodec) codec = fl_standard_method_codec_new();
  g_autoptr(FlMethodChannel) channel = fl_method_channel_new(fl_plugin_registrar_get_messenger(registrar), "point_glass_opengl", FL_METHOD_CODEC(codec));
  fl_method_channel_set_method_call_handler(channel, method_call_cb, g_object_ref(plugin), g_object_unref);
  g_object_unref(plugin);
}