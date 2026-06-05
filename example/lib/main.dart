import 'dart:math';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';

import 'package:point_glass_opengl/point_glass_opengl.dart';

void main() {
  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({Key? key}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      home: Scaffold(
        appBar: AppBar(title: const Text('Point Glass OpenGL Test')),
        body: const PointGlassExample(),
      ),
    );
  }
}

class PointGlassExample extends StatefulWidget {
  const PointGlassExample({super.key});

  @override
  State<PointGlassExample> createState() => _PointGlassExampleState();
}

class _PointGlassExampleState extends State<PointGlassExample> {
  final PointGlassController _controller = PointGlassController();

  bool _isShiftPressed = false;
  bool _isCtrlPressed = false;

  void _loadScene() {
    // 1. 선(Lines): 회색 격자 (Grid) 생성
    const int count = 10;
    const double half = count / 2.0;
    final List<double> lineVerts = [];

    // 헬퍼: [X, Y, Z, R, G, B, A, 두께]
    void addLineVert(double x, double y, double z) {
      lineVerts.addAll([x, y, z, 0.4, 0.4, 0.4, 1.0, 1.0]); // 회색 선
    }

    for (int i = 0; i <= count; i++) {
      double t = -half + i.toDouble();
      addLineVert(-half, t, 0.0);
      addLineVert(half, t, 0.0);
      addLineVert(t, -half, 0.0);
      addLineVert(t, half, 0.0);
    }
    _controller.setLines(Float32List.fromList(lineVerts));

    // 2. 면(Polygons): 중앙에 반투명한 빨간색 삼각 텐트 그리기
    final List<double> polyVerts = [
      // X, Y, Z,       R, G, B, A,          Reserved(크기 무시)
      1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.5, 1.0, // 윗점 (반투명 빨강)
      -1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.5, 1.0, // 아래 왼쪽
      0.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.5, 1.0, // 아래 오른쪽
    ];
    _controller.setPolygons(Float32List.fromList(polyVerts));

    // 3. 점(Points): 무작위 위치에 다양한 색상과 크기의 점 500개 뿌리기
    final List<double> pointVerts = [];
    final rand = Random();
    for (int i = 0; i < 500; i++) {
      double x = (rand.nextDouble() * 10) - 5.0;
      double y = (rand.nextDouble() * 10) - 5.0;
      double z = (rand.nextDouble() * 5);

      // 색상과 크기(3.0 ~ 8.0)도 무작위
      pointVerts.addAll([
        x,
        y,
        z,
        rand.nextDouble(),
        rand.nextDouble(),
        rand.nextDouble(),
        1.0,
        (rand.nextDouble() * 5.0) + 3.0,
      ]);
    }
    _controller.setPoints(Float32List.fromList(pointVerts));
  }

  @override
  void initState() {
    super.initState();
    ServicesBinding.instance.keyboard.addHandler(_handleKeyEvent);
  }

  bool _handleKeyEvent(KeyEvent event) {
    if (event.logicalKey == LogicalKeyboardKey.shiftLeft ||
        event.logicalKey == LogicalKeyboardKey.shiftRight) {
      setState(() {
        _isShiftPressed = event is KeyDownEvent || event is KeyRepeatEvent;
      });
      return true;
    } else if (event.logicalKey == LogicalKeyboardKey.controlLeft ||
        event.logicalKey == LogicalKeyboardKey.controlRight) {
      setState(() {
        _isCtrlPressed = event is KeyDownEvent || event is KeyRepeatEvent;
      });
      return true;
    }
    return false;
  }

  @override
  void dispose() {
    ServicesBinding.instance.keyboard.removeHandler(_handleKeyEvent);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        Expanded(
          child: Padding(
            padding: const EdgeInsets.all(16.0),
            child: Listener(
              onPointerSignal: (event) {
                if (event is PointerScrollEvent) {
                  final scaleFactor = event.scrollDelta.dy > 0 ? 1.1 : 0.9;
                  _controller.changeCameraZoom(scaleFactor);
                }
              },
              onPointerMove: (event) {
                if (_isShiftPressed) {
                  _controller.panCamera(-event.delta.dx, event.delta.dy);
                } else if (_isCtrlPressed) {
                  _controller.rollCamera(-event.delta.dx);
                } else if (event.buttons == kPrimaryMouseButton) {
                  _controller.changeCameraAngle(
                    -event.delta.dx,
                    event.delta.dy,
                  );
                }
              },
              child: PointGlassView(
                controller: _controller,
                onInitialized: _loadScene,
              ),
            ),
          ),
        ),
      ],
    );
  }
}
