import 'dart:async';
import 'dart:ffi' hide Size;
import 'dart:io';
import 'dart:math';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter/services.dart';

import 'package:ffi/ffi.dart';
import 'package:vector_math/vector_math.dart' as vm;

import 'package:point_glass_opengl/src/models/point_glass_opengl_points_color_mode.dart';

import 'point_glass_opengl_native_bindings.dart';

// ============================================================================
// PointGlassOpenGLController
// ============================================================================
//
// 사용자가 3D viewer를 제어하기 위한 controller.
//
// Rust Renderer pointer와 Flutter texture ID를 보관하고,
// scene data 전송, render 요청, camera 제어를 담당.
class PointGlassOpenGLController with ChangeNotifier {
  // Native plugin에 texture 생성, 크기 변경, render를 요청하는 channel
  static const MethodChannel _channel = MethodChannel('point_glass_opengl');

  // Native library와 Rust FFI 함수 연결
  late PointGlassOpenGLNativeBindings _bindings;

  // Rust에서 생성한 Renderer의 native pointer
  Pointer<Void>? _rendererPtr;

  // Flutter engine에서 발급한 texture ID
  int? textureId;

  // OpenGL clear color 배경색
  Color bgColor = const Color(0xFF1A1A1A);

  // 같은 Flutter frame 안의 여러 render 요청을 하나로 병합하기 위한 상태.
  // Native render가 진행 중일 때 발생한 추가 요청은 완료 후 다음 frame에 처리.
  bool _renderScheduled = false;
  bool _renderInProgress = false;
  bool _renderRequestedWhileBusy = false;
  bool _notifyListenersPending = false;

  // Camera 좌우 orbit 각도
  double yaw = pi;

  // Camera 상하 orbit 각도
  double pitch = pi;

  // Z축 기준 camera 회전 각도
  double roll = 0.0;

  // Camera와 target 사이의 거리
  double radius = 8.0;

  // ============================================================================
  // Initialization
  // ============================================================================

  /// Native library와 FFI 함수를 초기화하고 render texture 생성.
  ///
  /// [width], [height]는 최초 Renderer와 texture 크기로 사용.
  Future<void> initialize({int width = 400, int height = 400}) async {
    _bindings = PointGlassOpenGLNativeBindings.load();

    // Rust Renderer 생성 후 초기 camera와 viewport 상태 전달
    _rendererPtr = _bindings.createRenderer();
    _bindings.updateCamera(
      _rendererPtr!,
      yaw,
      pitch,
      roll,
      radius,
    );
    _bindings.resizeRenderer(
      _rendererPtr!,
      width,
      height,
    );
    _bindings.setClearColor(
      _rendererPtr!,
      bgColor.r,
      bgColor.g,
      bgColor.b,
      bgColor.a,
    );

    // Native plugin에 Renderer 주소, render 함수 주소, texture 크기 전달
    textureId = await _channel.invokeMethod<int>('createTexture', {
      'rendererPtr': _rendererPtr!.address,
      'renderFuncPtr': _bindings.renderFunctionAddress,
      'width': width,
      'height': height,
    });
  }

  /// OpenGL clear color 배경색을 변경.
  ///
  /// [color]는 RGBA로 지정.
  void setBgColor(Color color) {
    bgColor = color;

    if (_rendererPtr == null) {
      return;
    }

    _bindings.setClearColor(
      _rendererPtr!,
      color.r,
      color.g,
      color.b,
      color.a,
    );
    render();
  }

  // ============================================================================
  // Coordinate conversion
  // ============================================================================

  /// 여러 3D 좌표를 화면 표시용 NDC 좌표로 일괄 변환.
  ///
  /// Camera 뒤쪽이거나 투영할 수 없는 좌표는 `null`로 반환.
  List<Offset?> project3DToScreenBatch(List<vm.Vector3> points) {
    if (_rendererPtr == null || points.isEmpty) return [];

    final count = points.length;

    // FFI 호출에 사용할 native input/output buffer 할당
    final inPtr = calloc<Float>(count * 3);
    final outPtr = calloc<Float>(count * 2);

    // Dart Vector3 값을 native input buffer로 복사
    for (int i = 0; i < count; i++) {
      inPtr[i * 3] = points[i].x;
      inPtr[i * 3 + 1] = points[i].y;
      inPtr[i * 3 + 2] = points[i].z;
    }

    // Rust에서 입력 좌표를 일괄 투영
    _bindings.project3DToScreenBatch(
      _rendererPtr!,
      inPtr,
      count,
      outPtr,
    );

    // Native output을 Dart Offset 목록으로 변환
    final results = <Offset?>[];

    for (int i = 0; i < count; i++) {
      final sx = outPtr[i * 2];
      final sy = outPtr[i * 2 + 1];

      if (sx < -9000.0) {
        // Camera 뒤쪽이거나 투영할 수 없는 좌표
        results.add(null);
      } else {
        // Pixel 좌표로 변환하기 전의 NDC 좌표
        results.add(Offset(sx, sy));
      }
    }

    // FFI 호출에 사용한 native buffer 해제
    calloc.free(inPtr);
    calloc.free(outPtr);

    return results;
  }

  /// 화면 좌표를 지정한 Z 평면의 world 좌표로 변환.
  ///
  /// Ray가 평면과 교차하지 않거나 좌표 변환에 실패하면 `null` 반환.
  vm.Vector3? screenToWorldOnPlane({
    required Offset screenPosition,
    required Size viewportSize,
    double planeZ = 0.0,
  }) {
    if (_rendererPtr == null ||
        viewportSize.width <= 0.0 ||
        viewportSize.height <= 0.0) {
      return null;
    }

    // Flutter local pixel X 좌표를 NDC 범위로 변환
    final ndcX = (screenPosition.dx / viewportSize.width) * 2.0 - 1.0;

    // `project3DToScreenBatch()`의 screen 변환과 동일한 기준으로
    // Flutter local pixel Y 좌표를 NDC 범위로 역변환
    final ndcY = (screenPosition.dy / viewportSize.height) * 2.0 - 1.0;

    final outPtr = calloc<Float>(3);

    try {
      final result = _bindings.screenToWorldOnPlane(
        _rendererPtr!,
        ndcX,
        ndcY,
        planeZ,
        outPtr,
      );

      if (result == 0) {
        return null;
      }

      return vm.Vector3(
        outPtr[0],
        outPtr[1],
        outPtr[2],
      );
    } finally {
      calloc.free(outPtr);
    }
  }

  // ============================================================================
  // Rendering
  // ============================================================================

  /// Renderer와 native texture 크기를 갱신하고 화면 갱신을 예약.
  Future<void> resize(int width, int height) async {
    if (textureId == null || _rendererPtr == null) {
      return;
    }

    // Rust Renderer와 native texture에 새로운 크기 전달
    _bindings.resizeRenderer(
      _rendererPtr!,
      width,
      height,
    );
    await _channel.invokeMethod('resizeTexture', {
      'width': width,
      'height': height,
    });
    render();
  }

  /// 다음 Flutter frame에 화면 갱신을 예약.
  ///
  /// 같은 frame 안에서 여러 번 호출되면 하나의 render 요청으로 병합.
  /// [notify]가 `true`이면 render 완료 후 listener 갱신도 함께 예약.
  void render({bool notify = true}) {
    if (_rendererPtr == null || textureId == null) {
      return;
    }

    _notifyListenersPending |= notify;

    // 이미 다음 frame render가 예약된 경우 추가 callback을 등록하지 않음.
    // 이후 변경된 scene과 camera 상태는 예약된 render에서 함께 반영.
    if (_renderScheduled) {
      return;
    }

    // Native render가 진행 중이면 완료 후 추가 render가 필요함을 기록
    if (_renderInProgress) {
      _renderRequestedWhileBusy = true;
      return;
    }

    _renderScheduled = true;

    SchedulerBinding.instance.scheduleFrameCallback((_) {
      _renderScheduled = false;
      unawaited(_performRender());
    });
  }

  // 예약된 native render를 실제 수행.
  Future<void> _performRender() async {
    final renderer = _rendererPtr;

    if (renderer == null || textureId == null) {
      return;
    }

    if (_renderInProgress) {
      _renderRequestedWhileBusy = true;
      return;
    }

    _renderInProgress = true;

    // 현재 render 요청까지 누적된 overlay 갱신 여부 확보
    final shouldNotifyListeners = _notifyListenersPending;
    _notifyListenersPending = false;

    try {
      if (Platform.isAndroid) {
        // Android는 SurfaceTexture의 EGL surface에 Rust가 직접 render.
        // MethodChannel 경로를 함께 호출하면 중복 render가 발생하므로 FFI만 사용.
        _bindings.renderFrame(renderer);
      } else {
        // Windows/Linux는 native plugin의 render callback 호출
        await _channel.invokeMethod<void>('requestRender');
      }
    } finally {
      _renderInProgress = false;

      if (shouldNotifyListeners) {
        notifyListeners();
      }

      // Native render 중 scene 또는 overlay 상태가 다시 변경된 경우
      // 다음 Flutter frame에 추가 render 예약.
      if (_renderRequestedWhileBusy || _notifyListenersPending) {
        _renderRequestedWhileBusy = false;
        render(notify: false);
      }
    }
  }

  // `Float32List`를 native buffer로 복사한 뒤 Rust에 전달.
  //
  // 빈 배열도 length 0으로 전달하여 기존 GPU data를 제거할 수 있게 함.
  // Renderer가 준비되지 않은 경우 `false`, 전달한 경우 `true` 반환.
  bool _sendDataToRust(
    void Function(Pointer<Void>, Pointer<Float>, int) ffiFunction,
    Float32List data,
  ) {
    final renderer = _rendererPtr;

    if (renderer == null) {
      return false;
    }

    if (data.isEmpty) {
      ffiFunction(renderer, nullptr.cast<Float>(), 0);
      return true;
    }

    final pointer = calloc<Float>(data.length);

    try {
      pointer.asTypedList(data.length).setAll(0, data);
      ffiFunction(renderer, pointer, data.length);
      return true;
    } finally {
      calloc.free(pointer);
    }
  }

  // ============================================================================
  // Scene data
  // ============================================================================

  /// Point cloud 데이터를 갱신하고 화면 갱신을 예약.
  ///
  /// 데이터 포맷:
  /// `[x, y, z, value, ...]`
  void setPoints(Float32List points) {
    if (_sendDataToRust(_bindings.setPoints, points)) {
      render();
    }
  }

  /// Line 데이터를 갱신하고 화면 갱신을 예약.
  ///
  /// 데이터 포맷:
  /// `[x, y, z, r, g, b, a, pad, ...]`
  void setLines(Float32List lines) {
    if (_sendDataToRust(_bindings.setLines, lines)) {
      render();
    }
  }

  /// Polygon 데이터를 갱신하고 화면 갱신을 예약.
  ///
  /// 데이터 포맷:
  /// `[x, y, z, r, g, b, a, pad, ...]`
  void setPolygons(Float32List polygons) {
    if (_sendDataToRust(_bindings.setPolygons, polygons)) {
      render();
    }
  }

  /// Point, line, polygon 데이터를 한 번에 갱신.
  ///
  /// 여러 FFI 호출이 발생해도 실제 render는 다음 Flutter frame에 한 번만 수행.
  void setSceneData({
    Float32List? points,
    Float32List? lines,
    Float32List? polygons,
  }) {
    var changed = false;

    if (points != null) {
      changed |= _sendDataToRust(_bindings.setPoints, points);
    }

    if (lines != null) {
      changed |= _sendDataToRust(_bindings.setLines, lines);
    }

    if (polygons != null) {
      changed |= _sendDataToRust(_bindings.setPolygons, polygons);
    }

    if (changed) {
      render();
    }
  }

  /// Point cloud 표시 옵션을 갱신하고 화면 갱신을 예약.
  void setPointCloudDisplayParams(
    double alpha,
    double size,
    double min,
    double max,
    PointGlassOpenGLPointsColorMode mode,
  ) {
    final renderer = _rendererPtr;

    if (renderer == null) {
      return;
    }

    _bindings.setPointCloudDisplayParams(
      renderer,
      alpha,
      size,
      min,
      max,
      mode.value,
    );

    render();
  }

  // ============================================================================
  // Camera control
  // ============================================================================

  /// Camera를 target 중심으로 좌우/상하 orbit.
  void changeCameraAngle(double deltaX, double deltaY) {
    final renderer = _rendererPtr;

    if (renderer == null) {
      return;
    }

    // Camera가 허용 범위를 넘어 뒤집히지 않도록 각도 제한
    yaw = (yaw - deltaX * 0.003).clamp(0, pi * 2).toDouble();
    pitch = (pitch - deltaY * 0.003).clamp(pi, pi * 2).toDouble();

    _bindings.updateCamera(
      renderer,
      yaw,
      pitch,
      roll,
      radius,
    );

    render();
  }

  /// Z축을 기준으로 camera roll 회전.
  void rollCamera(double deltaZ) {
    final renderer = _rendererPtr;

    if (renderer == null) {
      return;
    }

    roll = (roll + (deltaZ * 0.0015)) % (pi * 2);
    _bindings.updateCamera(
      renderer,
      yaw,
      pitch,
      roll,
      radius,
    );
    render();
  }

  /// 화면 drag 방향에 따라 camera target 이동.
  void panCamera(double screenDx, double screenDy) {
    final renderer = _rendererPtr;

    if (renderer == null) {
      return;
    }

    _bindings.panCamera(
      renderer,
      screenDx.toDouble(),
      screenDy.toDouble(),
    );
    render();
  }

  /// Camera와 target 사이의 거리를 비율로 조절.
  void changeCameraZoom(double scaleFactor) {
    final renderer = _rendererPtr;

    if (renderer == null) {
      return;
    }

    // Camera 거리가 0 이하가 되지 않도록 최소값 제한
    radius = (radius * scaleFactor).clamp(0.1, double.infinity);
    _bindings.updateCamera(
      renderer,
      yaw,
      pitch,
      roll,
      radius,
    );

    render();
  }
}
