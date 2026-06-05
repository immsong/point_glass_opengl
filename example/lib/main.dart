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

  // 10x10 격자 (XZ 평면, y=0)
  static Float32List _generateGridLines() {
    const int count = 10;
    const double half = count / 2.0;
    final List<double> verts = [];
    for (int i = 0; i <= count; i++) {
      final double t = -half + i.toDouble();
      verts.addAll([-half, 0.0, t, half, 0.0, t]); // X축 방향 선
      verts.addAll([t, 0.0, -half, t, 0.0, half]); // Z축 방향 선
    }
    return Float32List.fromList(verts);
  }

  void _loadGrid() {
    _controller.updatePoints(_generateGridLines());
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
    }
    if (event.logicalKey == LogicalKeyboardKey.controlLeft ||
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
                } else if (event.buttons == kPrimaryMouseButton) {
                  // if (_isCtrlPressed) {
                  _controller.changeCameraAngle(-event.delta.dx, 0.0);
                  // } else {
                  _controller.changeCameraAngle(0.0, event.delta.dy);
                  // }
                }
              },
              child: PointGlassView(
                controller: _controller,
                onInitialized: _loadGrid,
              ),
            ),
          ),
        ),
      ],
    );
  }
}
