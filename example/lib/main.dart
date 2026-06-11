import 'dart:math';

import 'package:flutter/material.dart';

import 'package:point_glass_opengl/point_glass_opengl.dart';
import 'package:vector_math/vector_math.dart' as vm;

void main() {
  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

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
  final List<PointGlassOpenGLPoints> pointsGroup = [];

  void _loadScene() {
    final rand = Random();
    for (int i = 0; i < 500; i++) {
      double x = (rand.nextDouble() * 10) - 5.0;
      double y = (rand.nextDouble() * 10) - 5.0;
      double z = (rand.nextDouble() * 5);

      // 색상과 크기(3.0 ~ 8.0)도 무작위
      pointsGroup.add(
        PointGlassOpenGLPoints(
          points: [vm.Vector3(x, y, z)],
          pointSize: (rand.nextDouble() * 5.0) + 3.0,
          color: Color.fromARGB(
            255,
            (rand.nextDouble() * 255).toInt(),
            (rand.nextDouble() * 255).toInt(),
            (rand.nextDouble() * 255).toInt(),
          ),
        ),
      );
    }
  }

  @override
  void initState() {
    super.initState();
    _loadScene();
  }

  @override
  void dispose() {
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        Expanded(
          child: Padding(
            padding: const EdgeInsets.all(16.0),
            child: PointGlassOpenGLViewer(pointsGroup: pointsGroup),
          ),
        ),
      ],
    );
  }
}
