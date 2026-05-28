import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:point_glass_opengl/point_glass_opengl_method_channel.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  MethodChannelPointGlassOpengl platform = MethodChannelPointGlassOpengl();
  const MethodChannel channel = MethodChannel('point_glass_opengl');

  setUp(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (MethodCall methodCall) async {
          return '42';
        });
  });

  tearDown(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, null);
  });

  test('getPlatformVersion', () async {
    expect(await platform.getPlatformVersion(), '42');
  });
}
