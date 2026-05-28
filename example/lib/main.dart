import 'package:flutter/material.dart';
import 'package:point_glass_opengl/point_glass_opengl.dart'; // 우리가 만든 플러그인 임포트

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
  // 플러그인의 텍스처와 Rust 렌더러를 관리할 컨트롤러 생성
  final PointGlassController _controller = PointGlassController();

  @override
  Widget build(BuildContext context) {
    return Center(
      child: SizedBox(
        width: 400,
        height: 400,
        // 컨트롤러를 주입하여 우리가 만든 Texture 도화지를 화면에 띄움
        child: PointGlassView(controller: _controller),
      ),
    );
  }
}
