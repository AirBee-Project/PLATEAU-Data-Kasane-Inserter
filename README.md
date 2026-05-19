# 開発方針

- 基本的な変換ロジックは[Nazori](https://github.com/AirBee-Project/Nazori)に依存させる
  - パースロジック,数学的ロジックなどはこのリポジトリには一切記述しない
  - NazoriはすべてのPLATEAUデータを変換できることを検証していないため、エラーがあってもPanicしない処理が大切
- このリポジトリではG空間情報センターからの取得、NazoriのFunctionの呼び出し、Kasaneへの挿入を行う
  - プロセス並列化
  - 非同期スケジュール調整
  - エラーの記録