import 'dart:math';
import 'dart:typed_data';

import 'package:flutter/material.dart';

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
    return Column(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        SizedBox(
          width: 400,
          height: 400,
          child: PointGlassView(controller: _controller),
        ),
        const SizedBox(height: 20),
        ElevatedButton(
          onPressed: () {
            // 버튼을 누르면 -1.0 ~ 1.0 공간에 랜덤한 3D 점 30,000개 생성
            final rand = Random();
            final points = Float32List(30000 * 3); // x, y, z
            for (int i = 0; i < points.length; i++) {
              points[i] = (rand.nextDouble() * 2.0) - 1.0;
            }

            // 컨트롤러를 통해 Rust로 전송!
            _controller.updatePoints(points);
          },
          child: const Text('Shoot 30,000 Points!'),
        ),
      ],
    );
  }
}
