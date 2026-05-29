import 'dart:math';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter/gestures.dart';

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

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          // 💡 렌더링 화면에 마우스 제어 이벤트 주입
          Listener(
            onPointerSignal: (pointerSignal) {
              if (pointerSignal is PointerScrollEvent) {
                // 스크롤로 줌 인/아웃
                _controller.changeCameraZoom(
                  pointerSignal.scrollDelta.dy * 0.01,
                );
              }
            },
            child: GestureDetector(
              onPanUpdate: (details) {
                // 드래그로 화면 회전
                _controller.changeCameraAngle(
                  details.delta.dx,
                  details.delta.dy,
                );
              },
              child: SizedBox(
                width: 500,
                height: 500,
                child: PointGlassView(controller: _controller),
              ),
            ),
          ),
          const SizedBox(height: 20),
          ElevatedButton(
            onPressed: () {
              final rand = Random();
              final points = Float32List(30000 * 3);
              for (int i = 0; i < points.length; i++) {
                points[i] = (rand.nextDouble() * 2.0) - 1.0;
              }
              _controller.updatePoints(points);
            },
            child: const Text('Shoot 30,000 Points!'),
          ),
        ],
      ),
    );
  }
}
