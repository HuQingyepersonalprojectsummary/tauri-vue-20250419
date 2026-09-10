import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';

const root = path.resolve(import.meta.dirname, '../../..');
const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'network-functional-native-'));
fs.mkdirSync(path.join(dir, 'src'));

let code = fs.readFileSync(path.join(root, 'src-tauri/src/platform.rs'), 'utf8');
function replace(start, end, replacement) {
  const a = code.indexOf(start), b = code.indexOf(end, a);
  assert.ok(a >= 0 && b > a, 'Production source boundary: ' + start);
  code = code.slice(0, a) + replacement + '\n' + code.slice(b);
}

replace('pub fn run_command_with_timeout(', '// 固定 PowerShell 脚本', `
pub fn run_command_with_timeout(_program: &Path, args: &[&str], input: Option<&[u8]>, _timeout: Duration) -> Result<ProcessOutput, String> {
 MOCK.with(|m| {
  let mut m = m.borrow_mut();
  let payload = input.map(|b| serde_json::from_slice::<serde_json::Value>(b).unwrap());
  m.calls.push(serde_json::json!({
    "args": if input.is_some() { vec!["mocked-powershell"] } else { args.to_vec() },
    "payload": payload
  }));
  if m.fail_v6_dns_once && args.get(1) == Some(&"ipv6") && args.contains(&"dnsservers") && args.contains(&"source=dhcp") {
    m.fail_v6_dns_once = false;
    return Err("fixture IPv6 DNS switch failed after DoH cleanup".into());
  }
  if let Some(p) = payload {
    for item in p["dohList"].as_array().unwrap() {
      let server_ip = item["serverIp"].as_str().unwrap();
      let action = item["action"].as_str().unwrap_or("set");
      if action == "restore" {
        // 模拟恢复逻辑：若有 interfaceSetting 则恢复单网卡条目
        if let Some(entry) = item.get("interfaceSetting") {
          if !entry.is_null() {
            m.snap.adapter_doh_settings.insert(server_ip.to_string(), serde_json::from_value(entry.clone()).unwrap());
          } else {
            m.snap.adapter_doh_settings.remove(server_ip);
          }
        }
        if let Some(global) = item.get("globalSetting") {
          if !global.is_null() {
            m.snap.doh_settings.insert(server_ip.to_string(), serde_json::from_value(global.clone()).unwrap());
          } else {
            m.snap.doh_settings.remove(server_ip);
          }
        }
      }
    }
  }
  Ok(ProcessOutput { success: true, stdout: String::new(), stderr: String::new() })
 })
}
`);

replace('pub fn get_adapter_snapshot(', '/// 辅助函数', `
pub fn get_adapter_snapshot(_target: &str) -> Result<AdapterSnapshot, String> {
  MOCK.with(|m| Ok(m.borrow().snap.clone()))
}
`);

const fixtures = `
struct Mock { snap: AdapterSnapshot, calls: Vec<serde_json::Value>, fail_v6_dns_once: bool }
fn initial() -> AdapterSnapshot {
 serde_json::from_value(serde_json::json!({
  "adapterName": "fixture",
  "interfaceIndex": 7,
  "interfaceGuid": "fixture-guid",
  "status": "Up",
  "dhcpEnabled": true,
  "dnsDhcpEnabled": false,
  "ipv6Enabled": false,
  "dohSupported": true,
  "hasInterfaceDoh": true,
  "adapterDohSettings": {
    "192.0.2.53": { "flags": 2, "template": "https://adapter.example/dns-query" }
  },
  "addresses": [],
  "gateways": [],
  "dnsServers": ["192.0.2.53"],
  "ip": "",
  "mask": "",
  "gateway": "",
  "dns1": "192.0.2.53",
  "dns2": "",
  "doh1": { "mode": "auto", "template": "https://adapter.example/dns-query", "allowFallback": false },
  "dohSettings": {
    "192.0.2.53": { "template": "https://global.example/dns-query", "autoUpgrade": true, "allowFallback": true }
  },
  "ipv6DhcpEnabled": true,
  "ipv6DnsDhcpEnabled": true
 })).unwrap()
}

thread_local! {
  static MOCK: std::cell::RefCell<Mock> = std::cell::RefCell::new(Mock {
    snap: initial(),
    calls: vec![],
    fail_v6_dns_once: false
  });
}

pub fn audit_main() {
  let mut rows = vec![];
  for name in [
    "disable_with_hidden_static",
    "disable_with_hidden_static_dns",
    "keep_dns_turns_doh_off",
    "static_dns_blank_keeps_previous",
    "rollback_uses_global_not_adapter_doh",
    "failed_ipv6_dns_does_not_restore_doh"
  ] {
    MOCK.with(|m| *m.borrow_mut() = Mock { snap: initial(), calls: vec![], fail_v6_dns_once: false });
    let mut cfg: Ipv4Config = serde_json::from_value(serde_json::json!({
      "adapter": "fixture",
      "ip": "",
      "mask": "",
      "gateway": "",
      "dns1": "",
      "dns2": "",
      "ipMode": "keep",
      "dnsMode": "keep",
      "ipv6Enabled": false
    })).unwrap();

    if name == "disable_with_hidden_static" {
      cfg.ipv6_mode = Some("static".into());
      cfg.ipv6_ip = "".into();
    }
    if name == "disable_with_hidden_static_dns" {
      cfg.ipv6_dns_mode = Some("static".into());
      cfg.ipv6_dns1 = "".into();
    }
    if name == "keep_dns_turns_doh_off" {
      cfg.dns_mode = Some("keep".into());
    }
    if name == "static_dns_blank_keeps_previous" {
      cfg.dns_mode = Some("static".into());
      cfg.dns1 = "".into();
      cfg.dns2 = "".into();
      MOCK.with(|m| m.borrow_mut().snap.dns_dhcp_enabled = true);
    }
    if name == "failed_ipv6_dns_does_not_restore_doh" {
      cfg.ipv6_enabled = Some(true);
      cfg.ipv6_dns_mode = Some("dhcp".into());
      cfg.dns_mode = Some("static".into());
      cfg.dns1 = "192.0.2.53".into();
      cfg.doh1 = initial().doh1;
      MOCK.with(|m| {
        let mut m = m.borrow_mut();
        m.fail_v6_dns_once = true;
        m.snap.ipv6_enabled = true;
        m.snap.ipv6_dns_dhcp_enabled = false;
        m.snap.ipv6_dns_servers = vec!["2001:db8::53".into()];
        m.snap.doh_settings.insert("2001:db8::53".into(), DohServerSetting {
          template: "https://v6.example/dns-query".into(),
          auto_upgrade: true,
          allow_fallback: false
        });
        m.snap.adapter_doh_settings.insert("2001:db8::53".into(), crate::domain::AdapterDohSetting {
          flags: Some(2),
          template: Some("https://v6-adapter.example/dns-query".into())
        });
      });
    }
    if name.starts_with("rollback_") {
      let mut before = initial();
      before.ipv6_dns_servers = vec!["2001:db8::53".into()];
      before.doh_settings.insert("2001:db8::53".into(), DohServerSetting {
        template: "https://v6.example/dns-query".into(),
        auto_upgrade: true,
        allow_fallback: false
      });
      before.adapter_doh_settings.insert("2001:db8::53".into(), crate::domain::AdapterDohSetting {
        flags: Some(2),
        template: Some("https://v6-adapter.example/dns-query".into())
      });
      let result = rollback_snapshot_internal("fixture", &before, &["192.0.2.53".into()], &[]);
      MOCK.with(|m| rows.push(serde_json::json!({
        "case": name,
        "before": before,
        "result": result,
        "calls": m.borrow().calls
      })));
    } else {
      let result = apply_adapter_ipv4_config_transactional(&cfg);
      MOCK.with(|m| rows.push(serde_json::json!({
        "case": name,
        "config": cfg,
        "result": result,
        "calls": m.borrow().calls
      })));
    }
  }
  println!("{}", serde_json::to_string_pretty(&rows).unwrap());
}
`;

code += fixtures;
fs.writeFileSync(path.join(dir, 'src/platform.rs'), code);
fs.copyFileSync(path.join(root, 'src-tauri/src/domain.rs'), path.join(dir, 'src/domain.rs'));
fs.writeFileSync(path.join(dir, 'src/main.rs'), '#![allow(dead_code,unused_imports)]\nmod domain; mod platform; fn main(){platform::audit_main();}');
fs.writeFileSync(path.join(dir, 'Cargo.toml'), '[package]\nname="functional-native-test"\nversion="0.0.0"\nedition="2021"\n[dependencies]\nserde={version="=1.0.219",features=["derive"]}\nserde_json="=1.0.140"\nencoding_rs="=0.8.35"\n');
fs.copyFileSync(path.join(root, 'src-tauri/Cargo.lock'), path.join(dir, 'Cargo.lock'));

const r = spawnSync('cargo', ['run', '--offline', '--quiet', '--manifest-path', path.join(dir, 'Cargo.toml')], { encoding: 'utf8', timeout: 120000 });
assert.equal(r.status, 0, r.stderr || String(r.error));

const rows = JSON.parse(r.stdout);

// FA-05: 关闭 IPv6 忽略隐藏无效字段
const c1 = rows.find(r => r.case === 'disable_with_hidden_static');
assert.ok(c1.result.Ok, '关闭 IPv6 即使填写了无效静态 IPv6 仍应成功应用');
assert.equal(c1.result.Ok.success, true);

const c2 = rows.find(r => r.case === 'disable_with_hidden_static_dns');
assert.ok(c2.result.Ok, '关闭 IPv6 即使填写了无效静态 IPv6 DNS 仍应成功应用');
assert.equal(c2.result.Ok.success, true);

// FA-01: 保持 DNS 模式下，原有 DoH 配置完全保留，不清理
const keep = rows.find(r => r.case === 'keep_dns_turns_doh_off');
assert.ok(keep.result.Ok, '保持 DNS 配置应成功应用');
assert.equal(keep.result.Ok.success, true);
assert.equal(keep.result.Ok.snapshot.doh1.mode, 'auto', '保持 DNS 下 DoH 模式应保持为 auto');
const dohOffCalls = keep.calls.filter(c => c.payload?.dohList?.some(d => d.mode === 'off' && d.serverIp === '192.0.2.53'));
assert.equal(dohOffCalls.length, 0, '保持 DNS 不得向原有 DoH 发送 off 指令');

// FA-07: 空白的手动 IPv4 DNS 在后端执行前被校验阻断，不产生调用
const blankDns = rows.find(r => r.case === 'static_dns_blank_keeps_previous');
assert.ok(blankDns.result.Err, '空的手动 IPv4 DNS 必须报错');
assert.match(blankDns.result.Err, /手动 IPv4 DNS 必须填写首选 DNS/);
assert.equal(blankDns.calls.length, 0, '校验失败不得启动任何系统写入');

// FA-02: 单网卡 DoH 回滚生成 restore 动作并保留单网卡原始配置
const restore = rows.find(r => r.case === 'rollback_uses_global_not_adapter_doh');
assert.ok(restore.result.Ok === null || restore.result.Ok, '回滚应成功返回');
const rbDohCall = restore.calls.find(c => c.payload?.dohList);
assert.ok(rbDohCall, '回滚必须包含 DoH 恢复命令');
const item192 = rbDohCall.payload.dohList.find(d => d.serverIp === '192.0.2.53');
assert.ok(item192, '回滚必须包含 192.0.2.53');
assert.equal(item192.action, 'restore', 'DoH 回滚必须使用 restore 动作');
assert.equal(item192.interfaceSetting?.template, 'https://adapter.example/dns-query', '单网卡模板必须保持原网卡值');
assert.equal(item192.interfaceSetting?.flags, 2, '单网卡 flags 必须保持原值');
assert.equal(item192.globalSetting?.template, 'https://global.example/dns-query', '全局模板必须来自全局配置');

// FA-03: IPv6 DNS 切换失败回滚时，必须对 IPv6 DNS 的 DoH 生成恢复
const v6 = rows.find(r => r.case === 'failed_ipv6_dns_does_not_restore_doh');
assert.ok(v6.result.Ok, '事务在失败后执行回滚');
const v6RbCall = v6.calls.find(c => c.payload?.dohList?.some(d => d.action === 'restore'));
assert.ok(v6RbCall, '回滚必须调用 restore DoH');
const v6Item = v6RbCall.payload.dohList.find(d => d.serverIp === '2001:db8::53');
assert.ok(v6Item, 'IPv6 DNS 必须在 DoH 恢复列表中');
assert.equal(v6Item.action, 'restore');
assert.equal(v6Item.interfaceSetting?.flags, 2);
assert.equal(v6Item.globalSetting?.template, 'https://v6.example/dns-query');
assert.equal(v6.result.Ok.rolledBack, true, 'IPv6 DNS 恢复后现场核验一致，rolledBack 应为 true');

console.log(`Passed 6 native functional regression cases:
1. 关闭 IPv6 忽略隐藏无效静态 IPv6 地址 (FA-05)
2. 关闭 IPv6 忽略隐藏无效静态 IPv6 DNS (FA-05)
3. 保持 DNS 意图下严格保留现有 DoH 不产生清理 (FA-01)
4. 空白手动 IPv4 DNS 在写入前严格阻断 (FA-07)
5. DoH 回滚使用 restore 动作独立还原单网卡与全局设置 (FA-02)
6. IPv6 DNS 失败回滚完整包含 IPv6 DNS DoH 恢复并核验成功 (FA-03)`);
