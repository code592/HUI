# 本地文档边界

<script>throw new Error('must never execute')</script>

<iframe src="https://example.com"></iframe>

<img src="https://example.com/tracking.png" onerror="alert('never')">

<img src="file:///etc/passwd">

[危险链接](javascript:alert(1))

普通正文应当仍然可见。
