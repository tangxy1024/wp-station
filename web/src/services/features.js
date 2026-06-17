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

/**
 * 获取接入概览页的输入源 / 输出源运行时摘要
 */
export async function fetchIntegrationRuntimeOverview() {
  const response = await httpRequest.get('/integration-overview/runtime');
  const payload = response?.sources ? response : response?.data || response || {};

  const normalizeItems = (items = []) =>
    (Array.isArray(items) ? items : []).map((item) => ({
      key: item?.key || '',
      title: item?.title || '',
      connect: item?.connect || '',
      typeKey: item?.type_key || '',
      typeLabel: item?.type_label || '',
      detail: item?.detail || '-',
    }));

  return {
    sources: normalizeItems(payload.sources),
    sinks: normalizeItems(payload.sinks),
    supportedSourceTypeCount:
      typeof payload.supported_source_type_count === 'number'
        ? payload.supported_source_type_count
        : 0,
    supportedSinkTypeCount:
      typeof payload.supported_sink_type_count === 'number'
        ? payload.supported_sink_type_count
        : 0,
  };
}
