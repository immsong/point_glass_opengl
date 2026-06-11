import 'dart:async';
import 'dart:math';
import 'dart:typed_data';

import 'package:flutter/foundation.dart';
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
  final PointGlassOpenGLController _glController = PointGlassOpenGLController();
  Timer? _sceneTimer;

  void _loadScene() {
    List<PointGlassOpenGLPoints> pointsGroup = [];

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

    final floatData = DataConverter.convertPointsGroup(pointsGroup);
    _glController.setPoints(floatData);
  }

  @override
  void initState() {
    super.initState();

    WidgetsBinding.instance.addPostFrameCallback((_) {
      _loadScene();

      _sceneTimer = Timer.periodic(const Duration(milliseconds: 300), (_) {
        if (!mounted) return;
        _loadScene();
      });
    });
  }

  @override
  void dispose() {
    _sceneTimer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        Expanded(
          child: Padding(
            padding: const EdgeInsets.all(16.0),
            child: PointGlassOpenGLViewer(controller: _glController),
          ),
        ),
      ],
    );
  }
}
