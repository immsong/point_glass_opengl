package io.github.immsong.point_glass_opengl

import android.view.Surface
import io.flutter.embedding.engine.plugins.FlutterPlugin
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import io.flutter.plugin.common.MethodChannel.MethodCallHandler
import io.flutter.plugin.common.MethodChannel.Result
import io.flutter.view.TextureRegistry

class PointGlassOpenglPlugin : FlutterPlugin, MethodCallHandler {
    private lateinit var channel: MethodChannel
    private lateinit var textureRegistry: TextureRegistry
    private var textureEntry: TextureRegistry.SurfaceTextureEntry? = null
    private var surface: Surface? = null

    init {
        System.loadLibrary("point_glass_opengl_core")
    }

    // ▼ 수정됨: Dart에서 넘겨준 rendererPtr(Long)를 같이 받아서 Rust로 넘깁니다!
    private external fun nativeSetSurface(rendererPtr: Long, surface: Surface)

    override fun onAttachedToEngine(flutterPluginBinding: FlutterPlugin.FlutterPluginBinding) {
        channel = MethodChannel(flutterPluginBinding.binaryMessenger, "point_glass_opengl")
        channel.setMethodCallHandler(this)
        textureRegistry = flutterPluginBinding.textureRegistry
    }

    override fun onMethodCall(call: MethodCall, result: Result) {
        if (call.method == "createTexture") {
            val width = call.argument<Int>("width") ?: 0
            val height = call.argument<Int>("height") ?: 0
            
            // Dart 코드의 'rendererPtr': _rendererPtr!.address 를 받아옵니다.
            // Dart의 C 포인터 주소는 64비트 환경에서 Long으로 들어옵니다.
            val rendererPtr = call.argument<Long>("rendererPtr") ?: 0L

            textureEntry?.release()

            textureEntry = textureRegistry.createSurfaceTexture()
            val surfaceTexture = textureEntry!!.surfaceTexture()
            surfaceTexture.setDefaultBufferSize(width, height)

            surface = Surface(surfaceTexture)

            // Rust 쪽으로 포인터 주소와 Surface 객체를 동시에 넘겨줍니다!
            surface?.let { nativeSetSurface(rendererPtr, it) }

            result.success(textureEntry!!.id())

        } else if (call.method == "resizeTexture") {
            // 기존 Dart 코드에 있는 resizeTexture 채널 대응
            val width = call.argument<Int>("width") ?: 0
            val height = call.argument<Int>("height") ?: 0
            textureEntry?.surfaceTexture()?.setDefaultBufferSize(width, height)
            result.success(null)
            
        } else if (call.method == "requestRender") {
            // 기존 Dart 코드에 있는 requestRender 채널 대응 
            // (안드로이드에서는 보통 Rust 쪽에서 EGL SwapBuffers를 직접 호출하므로 여기선 비워둬도 무방합니다)
            result.success(null)
            
        } else {
            result.notImplemented()
        }
    }

    override fun onDetachedFromEngine(binding: FlutterPlugin.FlutterPluginBinding) {
        channel.setMethodCallHandler(null)
        textureEntry?.release()
    }
}