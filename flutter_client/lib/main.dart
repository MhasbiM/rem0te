import 'package:flutter/material.dart';
import 'pages/connect_page.dart';

void main() {
  runApp(const Rem0teApp());
}

class Rem0teApp extends StatelessWidget {
  const Rem0teApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'rem0te',
      debugShowCheckedModeBanner: false,
      theme: ThemeData.dark().copyWith(
        colorScheme: ColorScheme.fromSeed(
          seedColor: Colors.blue,
          brightness: Brightness.dark,
        ),
      ),
      home: const ConnectPage(),
    );
  }
}
