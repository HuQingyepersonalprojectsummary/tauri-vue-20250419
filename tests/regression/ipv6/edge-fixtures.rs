// Stateful isolated address IO: snapshots reflect captured commands; no network access.
thread_local! {
 static EDGE: std::cell::RefCell<Option<AdapterSnapshot>> = const { std::cell::RefCell::new(None) };
 static EDGE_BEFORE: std::cell::RefCell<Option<AdapterSnapshot>> = const { std::cell::RefCell::new(None) };
 static EDGE_FAILED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
fn edge_run(scenario: &str, args: &[&str]) -> Result<ProcessOutput,String> {
 let ok=||Ok(ProcessOutput{success:true,stdout:String::new(),stderr:String::new()});
 if args.get(1)!=Some(&"ipv6") { return ok(); }
 if args.contains(&"dnsservers") && args.contains(&"2001:db8::54") {
  EDGE_FAILED.with(|f|f.set(true));
  return Err("fixture DNS failure after address writes".into());
 }
 let deleting=args.get(2)==Some(&"delete") && args.get(3)==Some(&"address");
 let adding=args.get(2)==Some(&"add") && args.get(3)==Some(&"address");
 let rollback=EDGE_FAILED.with(|f|f.get());
 if deleting && rollback && scenario=="edge_cleanup_denied" {return Err("fixture cleanup denied".into());}
 if deleting && rollback && scenario=="edge_residual_lies" {return ok();}
 EDGE.with(|cell| {
  let mut state=cell.borrow_mut();let s=state.as_mut().unwrap();
  if deleting {s.ipv6_addresses.retain(|a|!crate::domain::ipv6_addr_eq(&a.ip_address,args[5]));}
  if adding {
   let (ip,prefix)=args[5].rsplit_once('/').unwrap();
   s.ipv6_addresses.push(crate::domain::Ipv6AddressConfig {ip_address:ip.into(),prefix_length:prefix.parse().unwrap(),prefix_origin:"Manual".into(),suffix_origin:"Manual".into()});
  }
  if args.contains(&"routerdiscovery=disabled") {s.ipv6_dhcp_enabled=false;}
  if args.contains(&"routerdiscovery=enabled") {
   s.ipv6_dhcp_enabled=true;
   EDGE_BEFORE.with(|b| {
    for original in &b.borrow().as_ref().unwrap().ipv6_addresses {
     if original.is_automatic() && !s.ipv6_addresses.iter().any(|a|crate::domain::ipv6_addr_eq(&a.ip_address,&original.ip_address)) {
      let mut restored=original.clone();
      if scenario=="edge_dynamic_churn" {restored.ip_address="2001:db8::abcd".into();}
      s.ipv6_addresses.push(restored);
     }
    }
   });
  }
 });
 if adding && !rollback && scenario=="edge_add_timeout" {EDGE_FAILED.with(|f|f.set(true));return Err("fixture address committed then timed out".into());}
 ok()
}
fn edge_snapshot(_scenario: &str, _reads: usize) -> AdapterSnapshot {EDGE.with(|s|s.borrow().as_ref().unwrap().clone())}
pub fn audit_edge_main() {
 let mut rows=Vec::new();
 for scenario in ["edge_same_ip_prefix","edge_auto_same_ip","edge_auto_same_prefix","edge_dynamic_churn","edge_static_cleanup_ok","edge_cleanup_denied","edge_residual_lies","edge_add_timeout","edge_empty_static_dns","edge_whitespace_dns","edge_secondary_only_dns","edge_unknown_origin"] {
  MOCK.with(|m| *m.borrow_mut()=MockState{scenario:scenario.into(),..Default::default()});
  let mut before=v6_snapshot("base",1);
  before.ipv6_dhcp_enabled=scenario!="edge_static_cleanup_ok";
  if ["edge_auto_same_ip","edge_auto_same_prefix","edge_dynamic_churn","edge_residual_lies"].contains(&scenario) {before.ipv6_addresses[0].prefix_origin="RouterAdvertisement".into();before.ipv6_addresses[0].suffix_origin="Random".into();}
  if scenario=="edge_unknown_origin" {before.ipv6_addresses[0].prefix_origin.clear();}
  EDGE.with(|s|*s.borrow_mut()=Some(before.clone()));
  EDGE_BEFORE.with(|s|*s.borrow_mut()=Some(before.clone()));
  EDGE_FAILED.with(|f|f.set(false));
  let mut cfg:Ipv4Config=serde_json::from_value(serde_json::json!({"adapter":"fixture","ipMode":"keep","dnsMode":"keep","ipv6Mode":"static","ipv6Ip":"2001:db8::10","ipv6Prefix":if scenario=="edge_auto_same_prefix" {64} else {80},"ipv6DnsMode":"static","ipv6Dns1":"2001:db8::54"})).unwrap();
  if scenario=="edge_static_cleanup_ok" {cfg.ipv6_ip="2001:db8::20".into();}
  if scenario=="edge_empty_static_dns" {cfg.ipv6_dns1.clear();}
  if scenario=="edge_whitespace_dns" {cfg.ipv6_dns1=" \t ".into();}
  if scenario=="edge_secondary_only_dns" {cfg.ipv6_dns1.clear();cfg.ipv6_dns2="2001:db8::55".into();}
  let result=apply_adapter_ipv4_config_transactional(&cfg);
  MOCK.with(|m| rows.push(serde_json::json!({"case":scenario,"before":before,"request":cfg,"result":result,"calls":m.borrow().calls})));
 }
 println!("{}",serde_json::to_string_pretty(&rows).unwrap());
}
