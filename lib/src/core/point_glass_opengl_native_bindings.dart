import 'dart:ffi';
import 'dart:io';

// ============================================================================
// FFI type definitions
// ============================================================================
//
// Dart와 Rust C ABI 사이에서 사용하는 함수 signature 정의.
//
// `XXXC`:
// - `NativeFunction` lookup에 사용하는 C ABI signature
//
// `XXXDart`:
// - Dart에서 실제 호출할 때 사용하는 signature
typedef _CreateRendererC = Pointer<Void> Function();
typedef _CreateRendererDart = Pointer<Void> Function();

typedef _RenderFrameC = Void Function(Pointer<Void> renderer);
typedef _RenderFrameDart = void Function(Pointer<Void> renderer);

typedef _RenderToBufferC = Void Function(
  Pointer<Void> renderer,
  Pointer<Uint8> buffer,
  Uint32 width,
  Uint32 height,
);

// Point, line, polygon 배열을 Rust로 전달하는 공통 FFI signature.
// `Pointer<Float>`는 `Float32List`를 복사한 native buffer 주소.
typedef _SetDataC = Void Function(
  Pointer<Void> renderer,
  Pointer<Float> data,
  IntPtr length,
);
typedef _SetDataDart = void Function(
  Pointer<Void> renderer,
  Pointer<Float> data,
  int length,
);

typedef _UpdateCameraC = Void Function(
  Pointer<Void> renderer,
  Float yaw,
  Float pitch,
  Float roll,
  Float radius,
);
typedef _UpdateCameraDart = void Function(
  Pointer<Void> renderer,
  double yaw,
  double pitch,
  double roll,
  double radius,
);

typedef _ResizeRendererC = Void Function(
  Pointer<Void> renderer,
  Uint32 width,
  Uint32 height,
);
typedef _ResizeRendererDart = void Function(
  Pointer<Void> renderer,
  int width,
  int height,
);

typedef _PanCameraC = Void Function(
  Pointer<Void> renderer,
  Float dx,
  Float dy,
);
typedef _PanCameraDart = void Function(
  Pointer<Void> renderer,
  double dx,
  double dy,
);

typedef _Project3DToScreenBatchC = Void Function(
  Pointer<Void> renderer,
  Pointer<Float> inCoords,
  IntPtr count,
  Pointer<Float> outCoords,
);
typedef _Project3DToScreenBatchDart = void Function(
  Pointer<Void> renderer,
  Pointer<Float> inCoords,
  int count,
  Pointer<Float> outCoords,
);

typedef _SetPointCloudDisplayParamsC = Void Function(
  Pointer<Void> renderer,
  Float alpha,
  Float size,
  Float min,
  Float max,
  Int32 mode,
);
typedef _SetPointCloudDisplayParamsDart = void Function(
  Pointer<Void> renderer,
  double alpha,
  double size,
  double min,
  double max,
  int mode,
);

typedef _ScreenToWorldOnPlaneC = Uint8 Function(
  Pointer<Void> renderer,
  Float ndcX,
  Float ndcY,
  Float planeZ,
  Pointer<Float> outPosition,
);
typedef _ScreenToWorldOnPlaneDart = int Function(
  Pointer<Void> renderer,
  double ndcX,
  double ndcY,
  double planeZ,
  Pointer<Float> outPosition,
);

// ============================================================================
// PointGlassOpenGLNativeBindings
// ============================================================================
//
// Native library 로딩과 Rust FFI symbol lookup을 담당.
//
// Controller에는 native 함수 구현과 symbol 이름을 노출하지 않고,
// Dart 메서드 형태로 필요한 기능만 제공.
class PointGlassOpenGLNativeBindings {
  PointGlassOpenGLNativeBindings._({
    required _CreateRendererDart createRenderer,
    required _RenderFrameDart renderFrame,
    required _SetDataDart setPoints,
    required _SetDataDart setLines,
    required _SetDataDart setPolygons,
    required _UpdateCameraDart updateCamera,
    required _ResizeRendererDart resizeRenderer,
    required _PanCameraDart panCamera,
    required _Project3DToScreenBatchDart project3DToScreenBatch,
    required _ScreenToWorldOnPlaneDart screenToWorldOnPlane,
    required _SetPointCloudDisplayParamsDart setPointCloudDisplayParams,
    required this.renderFunctionAddress,
  })  : _createRenderer = createRenderer,
        _renderFrame = renderFrame,
        _setPoints = setPoints,
        _setLines = setLines,
        _setPolygons = setPolygons,
        _updateCamera = updateCamera,
        _resizeRenderer = resizeRenderer,
        _panCamera = panCamera,
        _project3DToScreenBatch = project3DToScreenBatch,
        _screenToWorldOnPlane = screenToWorldOnPlane,
        _setPointCloudDisplayParams = setPointCloudDisplayParams;

  final _CreateRendererDart _createRenderer;
  final _RenderFrameDart _renderFrame;

  final _SetDataDart _setPoints;
  final _SetDataDart _setLines;
  final _SetDataDart _setPolygons;

  final _UpdateCameraDart _updateCamera;
  final _ResizeRendererDart _resizeRenderer;
  final _PanCameraDart _panCamera;

  final _Project3DToScreenBatchDart _project3DToScreenBatch;
  final _ScreenToWorldOnPlaneDart _screenToWorldOnPlane;
  final _SetPointCloudDisplayParamsDart _setPointCloudDisplayParams;

  // Native plugin이 호출할 platform별 render 함수 주소
  final int renderFunctionAddress;

  /// 현재 OS에 맞는 native library를 로드하고 FFI 함수 연결.
  factory PointGlassOpenGLNativeBindings.load() {
    final library = _loadNativeLibrary();

    // Native symbol을 한 번 lookup하여 Dart 함수로 캐싱
    final createRenderer = library
        .lookup<NativeFunction<_CreateRendererC>>('create_renderer')
        .asFunction<_CreateRendererDart>();
    final setPoints = library
        .lookup<NativeFunction<_SetDataC>>('set_points')
        .asFunction<_SetDataDart>();
    final setLines = library
        .lookup<NativeFunction<_SetDataC>>('set_lines')
        .asFunction<_SetDataDart>();
    final setPolygons = library
        .lookup<NativeFunction<_SetDataC>>('set_polygons')
        .asFunction<_SetDataDart>();
    final updateCamera = library
        .lookup<NativeFunction<_UpdateCameraC>>('update_camera')
        .asFunction<_UpdateCameraDart>();
    final resizeRenderer = library
        .lookup<NativeFunction<_ResizeRendererC>>('resize_renderer')
        .asFunction<_ResizeRendererDart>();
    final panCamera = library
        .lookup<NativeFunction<_PanCameraC>>('pan_camera')
        .asFunction<_PanCameraDart>();
    final project3DToScreenBatch = library
        .lookup<NativeFunction<_Project3DToScreenBatchC>>(
          'project_3d_to_screen_batch',
        )
        .asFunction<_Project3DToScreenBatchDart>();
    final screenToWorldOnPlane = library
        .lookup<NativeFunction<_ScreenToWorldOnPlaneC>>(
          'screen_to_world_on_plane',
        )
        .asFunction<_ScreenToWorldOnPlaneDart>();
    final setPointCloudDisplayParams = library
        .lookup<NativeFunction<_SetPointCloudDisplayParamsC>>(
          'set_point_cloud_display_params',
        )
        .asFunction<_SetPointCloudDisplayParamsDart>();

    final renderFramePointer =
        library.lookup<NativeFunction<_RenderFrameC>>('render_frame');
    final renderFrame = renderFramePointer.asFunction<_RenderFrameDart>();

    // Native plugin이 호출할 render 함수 주소 선택.
    // Windows는 CPU buffer render 함수, Linux/Android는 direct render 함수 사용.
    final renderFunctionAddress = switch (Platform.operatingSystem) {
      'windows' => library
          .lookup<NativeFunction<_RenderToBufferC>>('render_to_buffer')
          .address,
      'linux' || 'android' => renderFramePointer.address,
      final os => throw UnsupportedError('Unsupported OS: $os'),
    };

    return PointGlassOpenGLNativeBindings._(
      createRenderer: createRenderer,
      renderFrame: renderFrame,
      setPoints: setPoints,
      setLines: setLines,
      setPolygons: setPolygons,
      updateCamera: updateCamera,
      resizeRenderer: resizeRenderer,
      panCamera: panCamera,
      project3DToScreenBatch: project3DToScreenBatch,
      screenToWorldOnPlane: screenToWorldOnPlane,
      setPointCloudDisplayParams: setPointCloudDisplayParams,
      renderFunctionAddress: renderFunctionAddress,
    );
  }

  // 현재 OS에 맞는 native library 로드.
  static DynamicLibrary _loadNativeLibrary() {
    const libName = 'point_glass_opengl_core';

    // Windows에서는 실행 파일 옆의 DLL 로드
    if (Platform.isWindows) {
      return DynamicLibrary.open('$libName.dll');
    }

    // Linux와 Android에서는 shared object 로드
    if (Platform.isLinux || Platform.isAndroid) {
      return DynamicLibrary.open('lib$libName.so');
    }

    // 지원하지 않는 OS는 초기화 중단
    throw UnsupportedError('Unsupported OS: ${Platform.operatingSystem}');
  }

  Pointer<Void> createRenderer() {
    return _createRenderer();
  }

  void renderFrame(Pointer<Void> renderer) {
    _renderFrame(renderer);
  }

  void setPoints(
    Pointer<Void> renderer,
    Pointer<Float> data,
    int length,
  ) {
    _setPoints(renderer, data, length);
  }

  void setLines(
    Pointer<Void> renderer,
    Pointer<Float> data,
    int length,
  ) {
    _setLines(renderer, data, length);
  }

  void setPolygons(
    Pointer<Void> renderer,
    Pointer<Float> data,
    int length,
  ) {
    _setPolygons(renderer, data, length);
  }

  void updateCamera(
    Pointer<Void> renderer,
    double yaw,
    double pitch,
    double roll,
    double radius,
  ) {
    _updateCamera(renderer, yaw, pitch, roll, radius);
  }

  void resizeRenderer(
    Pointer<Void> renderer,
    int width,
    int height,
  ) {
    _resizeRenderer(renderer, width, height);
  }

  void panCamera(
    Pointer<Void> renderer,
    double dx,
    double dy,
  ) {
    _panCamera(renderer, dx, dy);
  }

  void project3DToScreenBatch(
    Pointer<Void> renderer,
    Pointer<Float> inCoords,
    int count,
    Pointer<Float> outCoords,
  ) {
    _project3DToScreenBatch(renderer, inCoords, count, outCoords);
  }

  int screenToWorldOnPlane(
    Pointer<Void> renderer,
    double ndcX,
    double ndcY,
    double planeZ,
    Pointer<Float> outPosition,
  ) {
    return _screenToWorldOnPlane(
      renderer,
      ndcX,
      ndcY,
      planeZ,
      outPosition,
    );
  }

  void setPointCloudDisplayParams(
    Pointer<Void> renderer,
    double alpha,
    double size,
    double min,
    double max,
    int mode,
  ) {
    _setPointCloudDisplayParams(
      renderer,
      alpha,
      size,
      min,
      max,
      mode,
    );
  }
}
