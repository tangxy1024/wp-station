/**
 * 特性配置相关 service
 * 目前提供数据采集监控地址的读取能力
 */

import httpRequest from './request';

/**
 * 获取数据采集页面配置
 * @returns {Promise<{data_collect_url: string, default_data_collect_url: string}>}
 */
export async function fetchDataCollectConfig() {
  const response = await httpRequest.get('/features/config');
  const payload = response?.data_collect_url ? response : response?.data || response || {};

  return {
    data_collect_url: payload.data_collect_url || payload.default_data_collect_url || '',
    default_data_collect_url: payload.default_data_collect_url || payload.data_collect_url || '',
  };
}
