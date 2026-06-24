import 'dart:ffi';
import 'dart:io';
import 'dart:math';
import 'dart:typed_data';

import 'package:flutter/services.dart';
import 'package:flutter/material.dart';

import 'package:ffi/ffi.dart';
import 'package:vector_math/vector_math.dart' as vm;

import 'package:point_glass_opengl/src/models/point_glass_opengl_points_color_mode.dart';

// ============================================================================
// [FFI (Foreign Function Interface) 타입 정의]
// Dart와 Rust(C ABI) 간의 통신을 위해 C 언어 타입과 Dart 타입을 매칭합니다.
// - XXXC: Rust/C 쪽에서 기대하는 함수 시그니처
// - XXXDart: Dart 쪽에서 호출할 때 사용할 함수 시그니처
// ============================================================================
typedef CreateRendererC = Pointer<Void> Function();
typedef CreateRendererDart = Pointer<Void> Function();
typedef RenderFrameC = Void Function(Pointer<Void> renderer);
typedef RenderFrameDart = void Function(Pointer<Void> renderer);

typedef RenderToBufferC =
    Void Function(
      Pointer<Void> renderer,
      Pointer<Uint8> buffer,
      Uint32 width,
      Uint32 height,
    );

// 공통 데이터 전송 FFI (점, 선, 면 배열을 Rust로 넘길 때 사용)
// Pointer<Float>는 Dart의 Float32List가 변환된 네이티브 메모리 주소입니다.
typedef SetDataC =
    Void Function(Pointer<Void> renderer, Pointer<Float> data, IntPtr length);
typedef SetDataDart =
    void Function(Pointer<Void> renderer, Pointer<Float> data, int length);

typedef UpdateCameraC =
    Void Function(
      Pointer<Void> renderer,
      Float yaw,
      Float pitch,
      Float roll,
      Float radius,
    );
typedef UpdateCameraDart =
    void Function(
      Pointer<Void> renderer,
      double yaw,
      double pitch,
      double roll,
      double radius,
    );
typedef ResizeRendererC =
    Void Function(Pointer<Void> renderer, Uint32 width, Uint32 height);
typedef ResizeRendererDart =
    void Function(Pointer<Void> renderer, int width, int height);
typedef PanCameraC = Void Function(Pointer<Void> renderer, Float dx, Float dy);
typedef PanCameraDart =
    void Function(Pointer<Void> renderer, double dx, double dy);

typedef Project3DToScreenBatchC =
    Void Function(
      Pointer<Void> renderer,
      Pointer<Float> inCoords,
      IntPtr count,
      Pointer<Float> outCoords,
    );
typedef Project3DToScreenBatchDart =
    void Function(
      Pointer<Void> renderer,
      Pointer<Float> inCoords,
      int count,
      Pointer<Float> outCoords,
    );

typedef SetPointCloudDisplayParamsC =
    Void Function(
      Pointer<Void> renderer,
      Float alpha,
      Float size,
      Float min,
      Float max,
      Int32 mode,
    );
typedef SetPointCloudDisplayParamsDart =
    void Function(
      Pointer<Void> renderer,
      double alpha,
      double size,
      double min,
      double max,
      int mode,
    );

// ============================================================================
// [PointGlassController]
// 사용자가 3D 뷰어를 제어하기 위해 사용하는 핵심 컨트롤러 클래스입니다.
// Rust 렌더러 메모리 주소를 보관하고, 데이터 전송 및 카메라 조작 명령을 내립니다.
// ============================================================================
class PointGlassOpenGLController with ChangeNotifier {
  // Flutter 네이티브(C++) 쪽과 통신하여 텍스처(화면)를 생성/갱신하기 위한 채널
  static const MethodChannel _channel = MethodChannel('point_glass_opengl');

  Pointer<Void>? _rendererPtr; // Rust에서 생성된 Renderer 구조체의 메모리 주소
  int? textureId; // Flutter 엔진이 발급한 OpenGL Texture ID

  late DynamicLibrary _dylib;
  late CreateRendererDart _createRenderer;

  // 3가지 렌더링 데이터 바인딩 함수 (점, 선, 면)
  late SetDataDart _setPoints;
  late SetDataDart _setLines;
  late SetDataDart _setPolygons;

  late UpdateCameraDart _updateCamera;
  late ResizeRendererDart _resizeRenderer;
  late PanCameraDart _panCamera;

  late Project3DToScreenBatchDart _project3DToScreenBatch;

  late SetPointCloudDisplayParamsDart _setPointCloudDisplayParams;

  // --- 카메라 상태 변수 ---
  double yaw = pi; // 좌우 회전각 (Orbit)
  double pitch = pi; // 상하 회전각 (Orbit)
  double roll = 0.0; // Z축 기준 회전각 (Turn-table)
  double radius = 8.0; // 카메라와 목표 지점 사이의 거리 (Zoom)

  DynamicLibrary _loadNativeLibrary() {
    const String libName = 'point_glass_opengl_core';

    // 1. Windows
    // CMake가 빌드 시점에 .dll 파일을 .exe 실행 파일 바로 옆에 복사해 줍니다.
    // 따라서 파일 이름만 부르면 OS가 현재 폴더에서 바로 찾아냅니다.
    if (Platform.isWindows) {
      return DynamicLibrary.open('$libName.dll');
    }
    // 2. Ubuntu (Linux) & Android
    // Linux 역시 CMake가 빌드 결과물(lib 폴더)에 .so를 넣어주어 시스템이 인식하게 합니다.
    else if (Platform.isLinux || Platform.isAndroid) {
      return DynamicLibrary.open('lib$libName.so');
    }
    // 4. 예외 처리
    else {
      throw UnsupportedError('Unsupported OS: ${Platform.operatingSystem}');
    }
  }

  /// 플러그인 초기화: 네이티브 라이브러리를 로드하고 FFI 함수들을 연결합니다.
  Future<void> initialize({int width = 400, int height = 400}) async {
    // OS에 맞는 동적 라이브러리(.dll 또는 .so) 로드
    _dylib = _loadNativeLibrary();

    // FFI 함수 룩업 및 캐싱 (매 호출마다 룩업하면 느려지므로 초기화 시점에 수행)
    _createRenderer = _dylib
        .lookup<NativeFunction<CreateRendererC>>('create_renderer')
        .asFunction();
    _setPoints = _dylib
        .lookup<NativeFunction<SetDataC>>('set_points')
        .asFunction();
    _setLines = _dylib
        .lookup<NativeFunction<SetDataC>>('set_lines')
        .asFunction();
    _setPolygons = _dylib
        .lookup<NativeFunction<SetDataC>>('set_polygons')
        .asFunction();
    _updateCamera = _dylib
        .lookup<NativeFunction<UpdateCameraC>>('update_camera')
        .asFunction();
    _resizeRenderer = _dylib
        .lookup<NativeFunction<ResizeRendererC>>('resize_renderer')
        .asFunction();
    _panCamera = _dylib
        .lookup<NativeFunction<PanCameraC>>('pan_camera')
        .asFunction();
    _project3DToScreenBatch = _dylib
        .lookup<NativeFunction<Project3DToScreenBatchC>>(
          'project_3d_to_screen_batch',
        )
        .asFunction();
    _setPointCloudDisplayParams = _dylib
        .lookup<NativeFunction<SetPointCloudDisplayParamsC>>(
          'set_point_cloud_display_params',
        )
        .asFunction();

    // C++ 쪽으로 넘겨주기 위해 함수 포인터(주소) 자체를 추출
    final renderFuncPtr = switch (Platform.operatingSystem) {
      'windows' =>
        _dylib
            .lookup<NativeFunction<RenderToBufferC>>('render_to_buffer')
            .address,
      'linux' =>
        _dylib.lookup<NativeFunction<RenderFrameC>>('render_frame').address,
      final os => throw UnsupportedError('Unsupported OS: $os'),
    };

    // 1. Rust 렌더러 생성 및 초기 상태 설정
    _rendererPtr = _createRenderer();
    _updateCamera(_rendererPtr!, yaw, pitch, roll, radius);
    _resizeRenderer(_rendererPtr!, width, height);

    // 2. C++ 플러그인에 텍스처 생성 요청 (Rust 렌더러 주소와 해상도 전달)
    textureId = await _channel.invokeMethod<int>('createTexture', {
      'rendererPtr': _rendererPtr!.address,
      'renderFuncPtr': renderFuncPtr,
      'width': width,
      'height': height,
    });
  }

  /// 3D 좌표를 화면 좌표로 변환하여 반환합니다.
  List<Offset?> project3DToScreenBatch(List<vm.Vector3> points) {
    if (_rendererPtr == null || points.isEmpty) return [];

    final count = points.length;
    // C++ 힙 영역에 입력(3D)과 출력(2D) 메모리 할당
    final inPtr = calloc<Float>(count * 3);
    final outPtr = calloc<Float>(count * 2);

    // Dart Vector3 데이터를 C++ 메모리로 복사
    for (int i = 0; i < count; i++) {
      inPtr[i * 3] = points[i].x;
      inPtr[i * 3 + 1] = points[i].y;
      inPtr[i * 3 + 2] = points[i].z;
    }

    // Rust 마법(일괄 계산) 실행!
    _project3DToScreenBatch(_rendererPtr!, inPtr, count, outPtr);

    // 결과를 Dart의 Offset 리스트로 변환
    List<Offset?> results = [];
    for (int i = 0; i < count; i++) {
      final sx = outPtr[i * 2];
      final sy = outPtr[i * 2 + 1];

      if (sx < -9000.0) {
        results.add(null); // 화면 밖
      } else {
        results.add(Offset(sx, sy)); // NDC 비율
      }
    }

    // 메모리 누수 방지
    calloc.free(inPtr);
    calloc.free(outPtr);

    return results;
  }

  /// 창 크기 변경 시 호출되어 텍스처와 FBO 해상도를 업데이트합니다.
  Future<void> resize(int width, int height) async {
    if (textureId == null || _rendererPtr == null) {
      return;
    }

    // Rust와 C++ 양쪽에 새로운 해상도를 알리고 화면을 다시 그립니다.
    _resizeRenderer(_rendererPtr!, width, height);
    await _channel.invokeMethod('resizeTexture', {
      'width': width,
      'height': height,
    });
    render();
  }

  /// Flutter 프레임워크에 "화면을 다시 그려달라"고 요청합니다.
  void render() {
    if (textureId != null) {
      _channel.invokeMethod('requestRender');
    }

    notifyListeners(); // 라벨 등 Flutter 위젯이 업데이트되어야 할 때 사용
  }

  /// 내부 통신 헬퍼: Dart 배열(Float32List)을 C/Rust 메모리로 복사하여 전송합니다.
  void _sendDataToRust(SetDataDart ffiFunc, Float32List data) {
    if (_rendererPtr == null || data.isEmpty) {
      return;
    }

    // 1. C++ 힙 메모리 할당 (calloc)
    final pointer = calloc<Float>(data.length);
    // 2. Dart 데이터를 할당된 네이티브 메모리로 고속 복사
    pointer.asTypedList(data.length).setAll(0, data);
    // 3. Rust 함수 호출
    ffiFunc(_rendererPtr!, pointer, data.length);
    // 4. 메모리 누수(Leak) 방지를 위해 즉시 해제 (Rust 쪽에서 데이터를 복사해 갔으므로 안전함)
    calloc.free(pointer);
  }

  // ============================================================================
  // [사용자 공개 API] 점 데이터를 업데이트하고 즉시 화면을 갱신합니다.
  // 데이터 포맷: [X, Y, Z, Value] (정점 1개당 4개의 Float)
  // ============================================================================
  void setPoints(Float32List points) {
    _sendDataToRust(_setPoints, points);
    render();
  }

  // ============================================================================
  // [사용자 공개 API] 선, 면 데이터를 업데이트하고 즉시 화면을 갱신합니다.
  // 데이터 포맷: [X, Y, Z, R, G, B, A, Size/Thickness] (정점 1개당 8개의 Float)
  // ============================================================================
  void setLines(Float32List lines) {
    _sendDataToRust(_setLines, lines);
    render();
  }

  void setPolygons(Float32List polygons) {
    _sendDataToRust(_setPolygons, polygons);
    render();
  }

  void setPointCloudDisplayParams(
    double alpha,
    double size,
    double min,
    double max,
    PointGlassOpenGLPointsColorMode mode,
  ) {
    if (_rendererPtr != nullptr) {
      _setPointCloudDisplayParams(
        _rendererPtr!,
        alpha,
        size,
        min,
        max,
        mode.value,
      );
      render();
    }
  }

  // --- 카메라 제어 API ---

  /// 마우스 기본 드래그: 카메라를 대상(Target)을 중심으로 공전(Orbit) 시킵니다.
  void changeCameraAngle(double deltaX, double deltaY) {
    if (_rendererPtr == null) {
      return;
    }

    // % (pi * 2)를 사용하여 좌우로 360도 무한 회전이 가능하도록 처리
    yaw = (yaw - (deltaX * 0.0015)) % (pi * 2);
    // 상하 회전은 화면이 뒤집히지 않도록 일정 각도로 제한(clamp)
    pitch = (pitch - deltaY * 0.003).clamp(pi, pi * 2);
    _updateCamera(_rendererPtr!, yaw, pitch, roll, radius);
    render();
  }

  /// Ctrl + 드래그: Z축을 기준으로 3D 공간 전체를 턴테이블처럼 회전시킵니다.
  void rollCamera(double deltaZ) {
    if (_rendererPtr == null) {
      return;
    }

    roll = (roll + (deltaZ * 0.0015)) % (pi * 2);
    _updateCamera(_rendererPtr!, yaw, pitch, roll, radius);
    render();
  }

  /// Shift + 드래그: 화면 평면과 평행하게 카메라 기준점(Target)을 이동시킵니다.
  void panCamera(double screenDx, double screenDy) {
    if (_rendererPtr == null) {
      return;
    }

    _panCamera(_rendererPtr!, screenDx.toDouble(), screenDy.toDouble());
    render();
  }

  /// 휠 스크롤: 줌 인/아웃 (카메라와 대상 사이의 거리를 비율로 조절)
  void changeCameraZoom(double scaleFactor) {
    if (_rendererPtr == null) {
      return;
    }

    // 거리가 0 이하가 되어 에러가 나지 않도록 최소 0.1로 제한
    radius = (radius * scaleFactor).clamp(0.1, double.infinity);
    _updateCamera(_rendererPtr!, yaw, pitch, roll, radius);
    render();
  }
}
