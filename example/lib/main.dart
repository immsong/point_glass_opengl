import 'dart:async';
import 'dart:math';
import 'dart:typed_data';

import 'package:flutter/material.dart';

import 'package:point_glass_opengl/point_glass_opengl.dart';

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

  static const int _totalPoints = 500;
  static const int _batchSize = 100;

  void _loadScene() {
    if (_glController.textureId == null) {
      return;
    }

    final rand = Random();

    // Point cloud 데이터 포맷: [x, y, z, value]
    final floatData = Float32List(_totalPoints * 4);
    final batches = <PointGlassOpenGLPointBatch>[];

    final colorModes = PointGlassOpenGLPointsColorMode.values;

    // 500개 Point를 100개씩 5개의 Batch로 분리
    for (int first = 0; first < _totalPoints; first += _batchSize) {
      final batchIndex = first ~/ _batchSize;
      final count = min(_batchSize, _totalPoints - first);

      for (int i = 0; i < count; i++) {
        final index = first + i;
        final offset = index * 4;

        // Batch별 위치를 분리하여 Color Map 차이를 확인
        final x = (batchIndex - 2) * 2.0 + rand.nextDouble() * 1.5 - 0.75;
        final y = rand.nextDouble() * 2.0 - 1.0;
        final z = rand.nextDouble() * 5.0;
        final value = rand.nextDouble();

        floatData[offset] = x;
        floatData[offset + 1] = y;
        floatData[offset + 2] = z;
        floatData[offset + 3] = value;
      }

      // Batch별 Point cloud 표시 설정
      batches.add(
        PointGlassOpenGLPointBatch(
          first: first,
          count: count,
          pointSize: 3.0 + batchIndex,
          opacity: 1.0,
          valueMin: 0.0,
          valueMax: 1.0,
          colorMode: colorModes[batchIndex % colorModes.length],
        ),
      );
    }

    _glController.setPointBatches(points: floatData, batches: batches);
  }

  @override
  void initState() {
    super.initState();

    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) {
        return;
      }

      _sceneTimer = Timer.periodic(const Duration(milliseconds: 300), (_) {
        if (!mounted) {
          return;
        }

        _loadScene();
      });

      _loadScene();
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
            child: PointGlassOpenGLViewer(
              controller: _glController,
              bgColor: Colors.black,
              grid: PointGlassOpenGLGrid(),
              axis: PointGlassOpenGLAxis(),
              enableJoystick: false,
            ),
          ),
        ),
      ],
    );
  }
}
