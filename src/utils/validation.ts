/**
 * 严格校验 IPv4 地址（必须有 4 段，0-255，且不能有前导零）
 * @param ip IP 地址字符串
 * @param allowEmpty 是否允许空字符串（用于可选字段）
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
    // 严格禁止前导零 (如 010.0.0.1)
    if (part.length > 1 && part.startsWith('0')) {
      return false;
    }
    const num = Number(part);
    return !isNaN(num) && Number.isInteger(num) && num >= 0 && num <= 255 && String(num) === part;
  });
}

/**
 * 将点分十进制 IPv4 转为 32 位无符号整数
 */
export function ipToNumber(ip: string): number {
  return ip
    .split('.')
    .reduce((acc, octet) => ((acc << 8) | Number(octet)) >>> 0, 0);
}

/**
 * 严格校验子网掩码（必须是合法的连续二进制 1，前缀在 0..=32 之间，拒绝 255.0.255.0 等非连续掩码）
 */
export function validateSubnetMask(mask: string): boolean {
  if (!validateIpAddress(mask, false)) {
    return false;
  }

  const maskNum = ipToNumber(mask);
  if (maskNum === 0) {
    return true; // 0.0.0.0
  }

  // 二进制转换为字符串检查是否为连续的 1 后跟连续的 0
  const bin = (maskNum >>> 0).toString(2).padStart(32, '0');
  return /^1*0*$/.test(bin);
}

/**
 * 校验网关是否与本机 IP 位于同一子网
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
