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

// --- 1. 커스텀 OpenGL Texture 클래스 정의 ---
G_DECLARE_FINAL_TYPE(PointGlassTexture, point_glass_texture, POINT_GLASS, TEXTURE, FlTextureGL)

struct _PointGlassTexture
{
  FlTextureGL parent_instance;
  void *renderer_ptr;
  void (*render_func)(void *);
  uint32_t tex_id;
  uint32_t fbo_id;
  uint32_t depth_rbo_id; // 💡 추가: 3D 원근감을 위한 깊이 버퍼
  uint32_t current_width;
  uint32_t current_height;
  gint desired_width;
  gint desired_height;
};

G_DEFINE_TYPE(PointGlassTexture, point_glass_texture, fl_texture_gl_get_type())

static void point_glass_texture_init(PointGlassTexture *self) {}

static gboolean point_glass_texture_populate(FlTextureGL *texture,
                                             uint32_t *target, uint32_t *name,
                                             uint32_t *width, uint32_t *height,
                                             GError **error)
{
  PointGlassTexture *pg_texture = POINT_GLASS_TEXTURE(texture);

  uint32_t dw = (uint32_t)g_atomic_int_get(&pg_texture->desired_width);
  uint32_t dh = (uint32_t)g_atomic_int_get(&pg_texture->desired_height);
  if (dw == 0)
    dw = 1;
  if (dh == 0)
    dh = 1;

  if (pg_texture->tex_id != 0 &&
      (pg_texture->current_width != dw || pg_texture->current_height != dh))
  {
    glDeleteFramebuffers(1, &pg_texture->fbo_id);
    glDeleteRenderbuffers(1, &pg_texture->depth_rbo_id); // 💡 삭제 시 반영
    glDeleteTextures(1, &pg_texture->tex_id);
    pg_texture->tex_id = 0;
    pg_texture->fbo_id = 0;
    pg_texture->depth_rbo_id = 0;
  }

  if (pg_texture->tex_id == 0)
  {
    g_print("[C++] (Re)creating OpenGL Texture & Depth Buffer %dx%d\n", dw, dh);

    glGenTextures(1, &pg_texture->tex_id);
    glBindTexture(GL_TEXTURE_2D, pg_texture->tex_id);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, dw, dh, 0, GL_RGBA, GL_UNSIGNED_BYTE, nullptr);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);

    // 💡 깊이(Depth) 렌더버퍼 생성
    glGenRenderbuffers(1, &pg_texture->depth_rbo_id);
    glBindRenderbuffer(GL_RENDERBUFFER, pg_texture->depth_rbo_id);
    glRenderbufferStorage(GL_RENDERBUFFER, GL_DEPTH_COMPONENT24, dw, dh);

    glGenFramebuffers(1, &pg_texture->fbo_id);
    glBindFramebuffer(GL_FRAMEBUFFER, pg_texture->fbo_id);
    glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, pg_texture->tex_id, 0);
    glFramebufferRenderbuffer(GL_FRAMEBUFFER, GL_DEPTH_ATTACHMENT, GL_RENDERBUFFER, pg_texture->depth_rbo_id); // 💡 FBO에 부착

    glBindFramebuffer(GL_FRAMEBUFFER, 0);

    pg_texture->current_width = dw;
    pg_texture->current_height = dh;
  }

  if (pg_texture->render_func && pg_texture->renderer_ptr)
  {
    GLint previous_fbo;
    glGetIntegerv(GL_FRAMEBUFFER_BINDING, &previous_fbo);

    glBindFramebuffer(GL_FRAMEBUFFER, pg_texture->fbo_id);
    glViewport(0, 0, dw, dh);

    pg_texture->render_func(pg_texture->renderer_ptr);

    glBindFramebuffer(GL_FRAMEBUFFER, previous_fbo);
  }

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
    if (self->current_texture)
    {
      FlTextureRegistrar *texture_registrar = fl_plugin_registrar_get_texture_registrar(self->registrar);
      fl_texture_registrar_mark_texture_frame_available(texture_registrar, FL_TEXTURE(self->current_texture));
    }
    response = FL_METHOD_RESPONSE(fl_method_success_response_new(nullptr));
  }
  else if (strcmp(method, "resizeTexture") == 0)
  {
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