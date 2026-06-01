import 'dart:math';
import 'dart:typed_data';
import 'dart:async';

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

  Timer? _streamingTimer;
  bool _isStreaming = false;
  double _time = 0.0;

  // 출렁이는 3D 파도(Wave) 데이터를 생성하는 함수
  Float32List _generateWavePoints(double timeOffset) {
    int count = 30000;
    final points = Float32List(count * 3);
    final rand = Random(42); // 위치가 무작위로 튀지 않도록 시드 고정

    for (int i = 0; i < count; i++) {
      // 넓게 퍼진 X, Z 좌표
      double x = (rand.nextDouble() * 4.0) - 2.0;
      double z = (rand.nextDouble() * 4.0) - 2.0;
      // 시간에 따라 출렁이는 Y(높이) 좌표 계산 (사인 파동)
      double y = sin(x * 3.0 + timeOffset) * cos(z * 3.0 + timeOffset) * 0.3;

      points[i * 3] = x;
      points[i * 3 + 1] = y;
      points[i * 3 + 2] = z;
    }
    return points;
  }

  void _toggleStreaming() {
    if (_isStreaming) {
      _streamingTimer?.cancel();
    } else {
      // 대략 30FPS (33ms) 속도로 끊임없이 데이터를 밀어넣습니다!
      _streamingTimer = Timer.periodic(const Duration(milliseconds: 33), (
        timer,
      ) {
        _time += 0.1;
        _controller.updatePoints(_generateWavePoints(_time));
      });
    }
    setState(() {
      _isStreaming = !_isStreaming;
    });
  }

  @override
  void dispose() {
    _streamingTimer?.cancel();
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
              onPointerSignal: (pointerSignal) {
                if (pointerSignal is PointerScrollEvent) {
                  _controller.changeCameraZoom(
                    pointerSignal.scrollDelta.dy * 0.01,
                  );
                }
              },
              child: GestureDetector(
                onPanUpdate: (details) {
                  _controller.changeCameraAngle(
                    details.delta.dx,
                    details.delta.dy,
                  );
                },
                child: AspectRatio(
                  aspectRatio: 1.0,
                  child: ClipRRect(
                    borderRadius: BorderRadius.circular(12),
                    child: PointGlassView(controller: _controller),
                  ),
                ),
              ),
            ),
          ),
        ),
        Padding(
          padding: const EdgeInsets.only(bottom: 30.0),
          child: ElevatedButton.icon(
            onPressed: _toggleStreaming,
            icon: Icon(_isStreaming ? Icons.stop : Icons.play_arrow),
            label: Text(
              _isStreaming
                  ? 'Stop LiDAR Stream'
                  : 'Start LiDAR Stream (30 FPS)',
            ),
            style: ElevatedButton.styleFrom(
              padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 16),
              backgroundColor: _isStreaming
                  ? Colors.red.shade900
                  : Colors.blue.shade900,
            ),
          ),
        ),
      ],
    );
  }
}
