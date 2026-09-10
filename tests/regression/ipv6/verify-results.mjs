import fs from 'node:fs';
import assert from 'node:assert/strict';
const read=name=>JSON.parse(fs.readFileSync(new URL('output/'+name,import.meta.url),'utf8').replace(/^\uFEFF/,''));
const n=read('native-results.json'),e=read('native-edge-results.json'),f=read('frontend-results.json'),p=read('powershell-results.json');
const row=(list,name)=>{const r=list.find(r=>r.case===name);assert.ok(r,`Missing ${name}`);return r;};
const addresses=r=>r.result.Ok.snapshot.ipv6Addresses;
const commands=r=>r.calls.filter(c=>c.args[1]==='ipv6'&&c.args[3]==='address').map(c=>c.args);
for(const name of ['edge_same_ip_prefix','edge_auto_same_ip','edge_auto_same_prefix','edge_dynamic_churn','edge_static_cleanup_ok','edge_add_timeout']) {
 const r=row(e,name);assert.equal(r.result.Ok.success,false,name);assert.equal(r.result.Ok.rolledBack,true,name);
 const cmd=commands(r);const adds=cmd.filter(a=>a[2]==='add');
 assert.ok(cmd.filter(a=>a[2]==='delete').length>=2,name+' compensates actual address writes');
 if(name==='edge_dynamic_churn') {
  assert.equal(addresses(r)[0].ipAddress,'2001:db8::abcd');assert.equal(adds.length,1);
 } else if(['edge_auto_same_ip','edge_auto_same_prefix'].includes(name)) {
  assert.equal(addresses(r)[0].prefixOrigin,'RouterAdvertisement');assert.equal(addresses(r)[0].suffixOrigin,'Random');
  assert.equal(adds.length,1,'Automatic origin must not be restored by adding a manual address');
 } else {
  assert.equal(addresses(r).length,1);assert.equal(addresses(r)[0].ipAddress,'2001:db8::10');
  assert.equal(addresses(r)[0].prefixLength,64);assert.equal(addresses(r)[0].prefixOrigin,'Manual');
  assert.ok(adds.some(a=>a.includes('2001:db8::10/64')));
 }
}
for(const name of ['edge_cleanup_denied','edge_residual_lies']) assert.equal(row(e,name).result.Ok.rolledBack,false);
for(const name of ['edge_empty_static_dns','edge_whitespace_dns','edge_secondary_only_dns','edge_unknown_origin']) {
 const r=row(e,name);assert.ok(r.result.Err);assert.equal(r.calls.length,0,name+' must not launch any write or rollback');
}
for(const name of ['v6_extra_dns','v6_delete_timeout','v6_missing_secondary_dns','v6_wrong_gateway_and_mode']) assert.equal(row(n,name).result.Ok.success,false);
assert.ok(!commands(row(n,'v6_delete_timeout')).some(a=>a[2]==='add'&&a.includes('2001:db8::20/64')));
for(const name of ['v6_dhcp_rollback_residual','v6_static_rollback_residual']) assert.equal(row(n,name).result.Ok.rolledBack,false);
assert.equal(row(n,'v6_equivalent_address').result.Ok.success,true);
assert.ok(row(n,'ipv6_rollback_nonzero_exit_ignored').result.Err);
const parsed=row(n,'actual_snapshot_parser_casing').result.Ok.ipv6Addresses[0];
assert.equal(parsed.ipAddress,'2001:db8::10');assert.equal(parsed.prefixOrigin,'Manual');assert.equal(parsed.suffixOrigin,'Manual');
for(const name of ['legacy_history_ipv6_intent','legacy_draft_round_trip']) {
 const payload=row(f,name).payload;assert.equal(payload.ipv6Mode,undefined);assert.equal(payload.ipv6DnsMode,undefined);
}
assert.equal(row(f,'ipv6_only_history_lost_on_reload').reloadedCount,1);
assert.equal(row(f,'empty_static_ipv6_dns_submitted').loadingBefore,false);
assert.equal(row(f,'empty_static_ipv6_dns_submitted').submitted,false);
for(const name of ['whitespace_dns','secondary_only_dns','valid_single_dns','valid_pair_dns','keep_empty_dns','dhcp_empty_dns']) {
 const r=row(f,name);assert.equal(r.submitted,r.expected,name);
}
for(const name of ['ipv6_route_error','ipv6_dns_error','ipv6_mode_error']) assert.equal(row(p,name).success,false);
const psAddress=row(p,'normal_snapshot').snapshot.ipv6Addresses[0];
assert.equal(psAddress.PrefixOrigin,'RouterAdvertisement');assert.equal(psAddress.SuffixOrigin,'Random');
console.log(`Passed: ${e.length} stateful transaction cases, prior IPv6 regressions and DNS validation.`);
