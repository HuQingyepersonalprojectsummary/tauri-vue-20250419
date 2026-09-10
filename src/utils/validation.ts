/**
 * 网络配置校验工具模块
 * 
 * 包含严谨的 IPv4、连续二进制子网掩码、网关子网可达性、IPv6 地址及前缀长度的前端校验算法。
 * 与后端 Rust `domain.rs` 保持 100% 规则对齐，杜绝脏数据提交到系统底层。
 */

/**
 * 严格校验 IPv4 地址合法性
 * 
 * 规则：
 * 1. 必须由英文句点分隔为恰好 4 段；
 * 2. 每段必须在 0 ~ 255 整数范围内；
 * 3. 严格禁止前导零（例如 `010.0.0.1` 视为非法，防止被部分底层库误按八进制解析）；
 * 4. 禁止非数字字符或超出范围的数值。
 * 
 * @param ip 待校验的 IP 地址字符串
 * @param allowEmpty 是否允许空字符串（用于可选的网关或 DNS 输入框）
 * @returns boolean 是否合法
 */
export function validateIpAddress(ip: string, allowEmpty = true): boolean {
  const trimmed = ip.trim();
  if (trimmed === '') {
    return allowEmpty;
  }

  const parts = trimmed.split('.');
  if (parts.length !== 4) {
    return false;
  }

  return parts.every(part => {
    if (part === '') return false;
    // 严格禁止多位数字以 '0' 开头（如 01, 002）
    if (part.length > 1 && part.startsWith('0')) {
      return false;
    }
    const num = Number(part);
    return !isNaN(num) && Number.isInteger(num) && num >= 0 && num <= 255 && String(num) === part;
  });
}

/**
 * 将点分十进制 IPv4 字符串转换为 32 位无符号整数
 * 
 * 通过连续左移与按位或操作，快速用于子网按位与 (&) 掩码计算。
 * 
 * @param ip 合法的 IPv4 地址字符串
 * @returns number 32 位无符号整数
 */
export function ipToNumber(ip: string): number {
  return ip
    .split('.')
    .reduce((acc, octet) => ((acc << 8) | Number(octet)) >>> 0, 0);
}

/**
 * 严格校验子网掩码合法性
 * 
 * 规则：
 * 1. 必须首先是合法的 IPv4 点分十进制字符串；
 * 2. 转换成 32 位二进制后，必须是连续的 '1' 紧随连续的 '0'（前缀长度范围 0..=32）；
 * 3. 坚决拦截诸如 `255.0.255.0` 或 `255.255.0.255` 等“孔洞掩码”，防止路由表损坏。
 * 
 * @param mask 待校验的子网掩码字符串 (例如 "255.255.255.0")
 * @returns boolean 是否为合法的连续子网掩码
 */
export function validateSubnetMask(mask: string): boolean {
  if (!validateIpAddress(mask, false)) {
    return false;
  }

  const maskNum = ipToNumber(mask);
  if (maskNum === 0) {
    return true; // 允许 0.0.0.0 作为全通掩码
  }

  // 将 32 位数字转为补足 32 位的二进制字符串，正则表达式验证连续 1 紧随连续 0
  const bin = (maskNum >>> 0).toString(2).padStart(32, '0');
  return /^1*0*$/.test(bin);
}

/**
 * 校验默认网关是否与本机 IP 位于同一个有效子网段
 * 
 * 原理：
 * 1. 若未填网关（可选），直接通过；
 * 2. 网关格式必须为有效 IPv4 地址；
 * 3. 网关地址不能与本机 IP 地址完全一致（同一地址冲突）；
 * 4. 逻辑校验：`本机 IP & 子网掩码 === 网关 IP & 子网掩码`，确保直连物理层可达。
 * 
 * @param ip 本机 IPv4 地址
 * @param mask 子网掩码
 * @param gateway 默认网关地址
 * @returns { valid: boolean, error?: string } 校验结果与错误描述
 */
export function validateGatewayInSubnet(ip: string, mask: string, gateway: string): { valid: boolean; error?: string } {
  if (!gateway.trim()) {
    return { valid: true };
  }

  if (!validateIpAddress(gateway, false)) {
    return { valid: false, error: '网关格式不正确，必须为有效的 IPv4 地址' };
  }

  if (ip.trim() === gateway.trim()) {
    return { valid: false, error: '网关地址不能与本机 IP 地址相同' };
  }

  const ipNum = ipToNumber(ip);
  const maskNum = ipToNumber(mask);
  const gwNum = ipToNumber(gateway);

  if ((ipNum & maskNum) !== (gwNum & maskNum)) {
    return { valid: false, error: '网关地址与本机 IP 不在同一个子网段' };
  }

  return { valid: true };
}

/**
 * 严格校验 IPv6 地址格式合法性
 * 
 * 支持标准规范：
 * 1. 完整标准格式：由 8 段 16 进制数字组成（如 `2001:0db8:85a3:0000:0000:8a2e:0370:7334`）；
 * 2. 简写压缩格式：允许使用且至多只能出现一次双冒号 `::` 缩略连续全零段（如 `2400:3200::1`、`fe80::1`）；
 * 3. 自动剔除并允许带有网络范围 ID（Zone Index / Scope ID，如 `fe80::1%12`）；
 * 4. 每段长度需在 1 ~ 4 位十六进制字符之间。
 * 
 * @param ip 待校验的 IPv6 地址字符串
 * @param allowEmpty 是否允许为空
 * @returns boolean 是否合法
 */
export function validateIpv6Address(ip: string, allowEmpty = true): boolean {
  const trimmed = ip.trim();
  if (trimmed === '') {
    return allowEmpty;
  }

  // 移除可选的链路本地作用域 ID (如 fe80::1%12 -> fe80::1)
  const cleanIp = trimmed.includes('%') ? trimmed.split('%')[0] : trimmed;

  // 双冒号 :: 只能出现至多一次
  const doubleColonIndex = cleanIp.indexOf('::');
  if (doubleColonIndex !== -1 && cleanIp.indexOf('::', doubleColonIndex + 2) !== -1) {
    return false;
  }

  if (doubleColonIndex !== -1) {
    // 包含双冒号压缩
    const [left, right] = cleanIp.split('::');
    const leftParts = left ? left.split(':') : [];
    const rightParts = right ? right.split(':') : [];
    if (leftParts.length + rightParts.length > 7) {
      return false;
    }
    const allParts = [...leftParts, ...rightParts];
    return allParts.every(p => /^[0-9a-fA-F]{1,4}$/.test(p));
  } else {
    // 未使用 :: 压缩，必须恰好为 8 段
    const parts = cleanIp.split(':');
    if (parts.length !== 8) {
      return false;
    }
    return parts.every(p => /^[0-9a-fA-F]{1,4}$/.test(p));
  }
}

/**
 * 校验 IPv6 前缀长度有效性
 * 
 * IPv6 子网前缀长度必须为 1 到 128 之间的正整数（常见的局域网 SLAAC 默认为 64）。
 * 
 * @param prefix 前缀数值或字符串
 * @returns boolean 是否合法
 */
export function validateIpv6Prefix(prefix: number | string | undefined | null): boolean {
  if (prefix === undefined || prefix === null || prefix === '') return false;
  const num = typeof prefix === 'number' ? prefix : Number(String(prefix).trim());
  return !isNaN(num) && Number.isInteger(num) && num >= 1 && num <= 128;
}
