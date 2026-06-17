import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Table, Tag, message } from 'antd';
import {
  ApartmentOutlined,
  ApiOutlined,
  DatabaseOutlined,
  DeploymentUnitOutlined,
  EnvironmentOutlined,
  FileTextOutlined,
  FolderOpenOutlined,
  NumberOutlined,
} from '@ant-design/icons';
import {
  fetchRuleConfig,
  fetchRuleFiles,
  RuleType,
} from '@/services/config';
import { fetchIntegrationRuntimeOverview } from '@/services/features';

const WPL_FETCH_PAGE_SIZE = 50;

const normalizeWplEntry = (value, parseFileName) => {
  if (value === undefined || value === null) {
    return '';
  }
  const trimmed = String(value).trim();
  if (!trimmed) {
    return '';
  }
  if (!trimmed.includes('/')) {
    return `${trimmed}/${parseFileName}`;
  }
  const [rulePart, ...restParts] = trimmed.split('/');
  const rule = (rulePart || '').trim();
  const sub = (restParts.join('/') || '').trim() || parseFileName;
  if (!rule) {
    return sub;
  }
  return `${rule}/${sub}`;
};

const normalizeWplList = (items, parseFileName) => {
  const deduped = new Set();
  (Array.isArray(items) ? items : []).forEach((item) => {
    const entry = normalizeWplEntry(item, parseFileName);
    if (entry) {
      deduped.add(entry);
    }
  });
  return Array.from(deduped);
};

const getWplEntryParts = (entry, parseFileName) => {
  const normalized = normalizeWplEntry(entry, parseFileName);
  if (!normalized) {
    return { rule: '', sub: '' };
  }
  const [rule, sub] = normalized.split('/');
  return {
    rule: (rule || '').trim(),
    sub: (sub || '').trim(),
  };
};

const isWplSampleEntry = (entry, parseFileName, sampleFileName) =>
  normalizeWplEntry(entry, parseFileName).endsWith(`/${sampleFileName}`);
const isIgnoredWplIdentifier = (value) =>
  String(value || '').trim().toLowerCase().startsWith('ignore');

const parseTagAttributes = (rawTag = '') => {
  const attributes = {};
  const pattern = /([a-zA-Z0-9_]+)\s*:\s*"([^"]*)"/g;
  let match = pattern.exec(rawTag);
  while (match) {
    attributes[match[1]] = match[2];
    match = pattern.exec(rawTag);
  }
  return attributes;
};

const stripTomlComment = (input = '') => {
  let inQuote = false;
  let escaped = false;
  let result = '';

  for (const char of input) {
    if (char === '"' && !escaped) {
      inQuote = !inQuote;
    }
    if (char === '#' && !inQuote) {
      break;
    }
    result += char;
    escaped = char === '\\' && !escaped;
    if (char !== '\\') {
      escaped = false;
    }
  }

  return result.trim();
};

const findAssignmentIndex = (line = '') => {
  let inQuote = false;
  let depth = 0;

  for (let index = 0; index < line.length; index += 1) {
    const char = line[index];
    if (char === '"') {
      inQuote = !inQuote;
      continue;
    }
    if (inQuote) {
      continue;
    }
    if (char === '{' || char === '[') {
      depth += 1;
      continue;
    }
    if (char === '}' || char === ']') {
      depth = Math.max(0, depth - 1);
      continue;
    }
    if (char === '=' && depth === 0) {
      return index;
    }
  }

  return -1;
};

const splitTopLevel = (input = '', delimiter = ',') => {
  const items = [];
  let current = '';
  let inQuote = false;
  let depth = 0;

  for (const char of input) {
    if (char === '"') {
      inQuote = !inQuote;
      current += char;
      continue;
    }
    if (!inQuote && (char === '{' || char === '[')) {
      depth += 1;
    } else if (!inQuote && (char === '}' || char === ']')) {
      depth = Math.max(0, depth - 1);
    }

    if (char === delimiter && !inQuote && depth === 0) {
      if (current.trim()) {
        items.push(current.trim());
      }
      current = '';
      continue;
    }

    current += char;
  }

  if (current.trim()) {
    items.push(current.trim());
  }

  return items;
};

const parseTomlValue = (rawValue = '') => {
  const value = rawValue.trim();
  if (!value) {
    return '';
  }
  if (
    (value.startsWith('"') && value.endsWith('"')) ||
    (value.startsWith("'") && value.endsWith("'"))
  ) {
    return value.slice(1, -1);
  }
  if (value === 'true') {
    return true;
  }
  if (value === 'false') {
    return false;
  }
  return value;
};

const parseInlineObject = (rawValue = '') => {
  const value = rawValue.trim();
  if (!value.startsWith('{') || !value.endsWith('}')) {
    return {};
  }

  const content = value.slice(1, -1).trim();
  if (!content) {
    return {};
  }

  return splitTopLevel(content).reduce((accumulator, item) => {
    const index = findAssignmentIndex(item);
    if (index < 0) {
      return accumulator;
    }
    const key = item.slice(0, index).trim();
    const nextValue = item.slice(index + 1).trim();
    if (!key) {
      return accumulator;
    }
    accumulator[key] = parseTomlValue(nextValue);
    return accumulator;
  }, {});
};

const assignTomlValue = (target, key, rawValue) => {
  if (!key) {
    return;
  }

  if (key === 'params' && rawValue.trim().startsWith('{')) {
    target.params = {
      ...(target.params || {}),
      ...parseInlineObject(rawValue),
    };
    return;
  }

  target[key] = parseTomlValue(rawValue);
};

const parseTomlBlocks = (content = '', blockHeader = '', paramsHeader = '') => {
  const blocks = [];
  const lines = String(content || '').split(/\r?\n/);
  let current = null;
  let inParams = false;

  lines.forEach((rawLine) => {
    const line = stripTomlComment(rawLine);
    if (!line) {
      return;
    }

    if (line === blockHeader) {
      if (current) {
        blocks.push(current);
      }
      current = { params: {} };
      inParams = false;
      return;
    }

    if (!current) {
      return;
    }

    if (line === paramsHeader) {
      inParams = true;
      return;
    }

    if (line.startsWith('[')) {
      inParams = false;
      return;
    }

    const assignmentIndex = findAssignmentIndex(line);
    if (assignmentIndex < 0) {
      return;
    }

    const key = line.slice(0, assignmentIndex).trim();
    const rawValue = line.slice(assignmentIndex + 1).trim();

    if (inParams) {
      assignTomlValue(current.params, key, rawValue);
      return;
    }

    assignTomlValue(current, key, rawValue);
  });

  if (current) {
    blocks.push(current);
  }

  return blocks;
};

const resolveConnectorMeta = (metaMap = {}, connect = '') => {
  const normalized = String(connect || '').trim();
  if (!normalized) {
    return {
      typeKey: '-',
      typeLabel: '-',
      defaultParams: {},
    };
  }

  const matched = metaMap[normalized];
  if (matched) {
    return matched;
  }

  return {
    typeKey: normalized.replace(/_(src|sink)$/i, '').replace(/_/g, '-'),
    typeLabel: normalized.replace(/_(src|sink)$/i, '').replace(/_/g, '-'),
    defaultParams: {},
  };
};

const getPreferredValue = (params = {}, keys = []) => {
  for (const key of keys) {
    const value = params?.[key];
    if (value !== undefined && value !== null && String(value).trim() !== '') {
      return String(value).trim();
    }
  }
  return '';
};

const joinDetailSegments = (items = []) => items.filter(Boolean).join('，');

const joinPathSegments = (base = '', file = '') => {
  const normalizedBase = String(base || '').trim();
  const normalizedFile = String(file || '').trim();

  if (!normalizedBase) {
    return normalizedFile;
  }
  if (!normalizedFile) {
    return normalizedBase;
  }
  if (normalizedFile.startsWith('/')) {
    return normalizedFile;
  }

  return `${normalizedBase.replace(/\/+$/, '')}/${normalizedFile.replace(/^\/+/, '')}`;
};

const splitHostPort = (rawValue = '') => {
  const value = String(rawValue || '').trim();
  if (!value) {
    return { host: '', port: '' };
  }

  const sanitized = value.replace(/^[a-z]+:\/\//i, '').split('/')[0].trim();
  if (!sanitized) {
    return { host: '', port: '' };
  }

  const index = sanitized.lastIndexOf(':');
  if (index <= 0 || index === sanitized.length - 1) {
    return { host: sanitized, port: '' };
  }

  return {
    host: sanitized.slice(0, index),
    port: sanitized.slice(index + 1),
  };
};

const parseConnectionString = (value = '') =>
  String(value || '')
    .split(';')
    .map((item) => item.trim())
    .filter(Boolean)
    .reduce((accumulator, item) => {
      const index = item.indexOf('=');
      if (index < 0) {
        return accumulator;
      }
      const key = item.slice(0, index).trim().toUpperCase();
      const nextValue = item.slice(index + 1).trim();
      if (key && nextValue) {
        accumulator[key] = nextValue;
      }
      return accumulator;
    }, {});

const buildEffectiveParams = (connectorMetaMap, connect = '', params = {}) => {
  const meta = resolveConnectorMeta(connectorMetaMap, connect);
  const merged = {
    ...(meta.defaultParams || {}),
    ...(params || {}),
  };

  if (merged.base === true) {
    merged.base = meta.defaultParams?.base || '';
  }

  return merged;
};

const inferConnectorType = (meta = {}, connect = '', params = {}) => {
  const normalizedConnect = String(connect || '').trim().toLowerCase();
  const protocol = getPreferredValue(params, ['protocol']).toLowerCase();
  let typeKey = String(meta.typeKey || '').trim().toLowerCase();
  let typeLabel = meta.typeLabel || typeKey || '-';

  if (typeKey === 'syslog') {
    if (protocol === 'udp' || normalizedConnect.includes('udp')) {
      typeKey = 'syslog-udp';
      typeLabel = 'Syslog UDP';
    } else if (protocol === 'tcp' || normalizedConnect.includes('tcp')) {
      typeKey = 'syslog-tcp';
      typeLabel = 'Syslog TCP';
    }
  }

  return {
    typeKey,
    typeLabel,
  };
};

const buildConnectorDetail = (connectorMetaMap, connect = '', params = {}) => {
  const meta = resolveConnectorMeta(connectorMetaMap, connect);
  const effectiveParams = buildEffectiveParams(connectorMetaMap, connect, params);
  const { typeKey, typeLabel } = inferConnectorType(meta, connect, effectiveParams);

  if (['syslog-udp', 'syslog-tcp', 'tcp', 'udp'].includes(typeKey)) {
    const addr = getPreferredValue(effectiveParams, ['addr', 'host']);
    const port = getPreferredValue(effectiveParams, ['port']);
    const protocol = getPreferredValue(effectiveParams, ['protocol']);
    return {
      typeLabel,
      detail:
        joinDetailSegments([
          addr ? `地址 ${addr}` : '',
          port ? `端口 ${port}` : '',
          protocol ? `协议 ${protocol}` : '',
        ]) || '-',
    };
  }

  if (['mysql', 'postgres', 'doris', 'clickhouse'].includes(typeKey)) {
    const endpoint = getPreferredValue(effectiveParams, ['endpoint', 'host']);
    const database = getPreferredValue(effectiveParams, ['database']);
    const table = getPreferredValue(effectiveParams, ['table']);
    const { host, port } = splitHostPort(endpoint);
    return {
      typeLabel,
      detail:
        joinDetailSegments([
          host ? `地址 ${host}` : endpoint ? `地址 ${endpoint}` : '',
          port ? `端口 ${port}` : '',
          database ? `数据库 ${database}` : '',
          table ? `数据表 ${table}` : '',
        ]) || '-',
    };
  }

  if (typeKey === 'dmdb') {
    const connectionString = getPreferredValue(effectiveParams, ['connection_string']);
    const endpoint = getPreferredValue(effectiveParams, ['endpoint']);
    const database = getPreferredValue(effectiveParams, ['database', 'schema']);
    const table = getPreferredValue(effectiveParams, ['table']);
    const connectionParams = parseConnectionString(connectionString);
    const { host, port } = splitHostPort(endpoint);
    const connectionHost = connectionParams.SERVER || host;
    const connectionPort = connectionParams.TCP_PORT || port;
    return {
      typeLabel,
      detail:
        joinDetailSegments([
          connectionHost ? `地址 ${connectionHost}` : endpoint ? `地址 ${endpoint}` : '',
          connectionPort ? `端口 ${connectionPort}` : '',
          database ? `数据库 ${database}` : '',
          table ? `数据表 ${table}` : '',
        ]) || '-',
    };
  }

  if (typeKey === 'file') {
    const base = getPreferredValue(effectiveParams, ['base', 'path']);
    const file = getPreferredValue(effectiveParams, ['file', 'file_path']);
    const normalizedPath = joinPathSegments(base, file);
    return {
      typeLabel,
      detail: normalizedPath ? `文件路径 ${normalizedPath}` : '-',
    };
  }

  if (typeKey === 'kafka') {
    const brokers = getPreferredValue(effectiveParams, ['brokers']);
    const topic = getPreferredValue(effectiveParams, ['topic']);
    return {
      typeLabel,
      detail:
        joinDetailSegments([
          brokers ? `地址 ${brokers}` : '',
          topic ? `Topic ${topic}` : '',
        ]) || '-',
    };
  }

  if (typeKey === 'elasticsearch') {
    const host = getPreferredValue(effectiveParams, ['host', 'endpoint']);
    const port = getPreferredValue(effectiveParams, ['port']);
    const index = getPreferredValue(effectiveParams, ['index']);
    const { host: endpointHost, port: endpointPort } = splitHostPort(host);
    return {
      typeLabel,
      detail:
        joinDetailSegments([
          endpointHost ? `地址 ${endpointHost}` : host ? `地址 ${host}` : '',
          port || endpointPort ? `端口 ${port || endpointPort}` : '',
          index ? `索引 ${index}` : '',
        ]) || '-',
    };
  }

  const endpoint = getPreferredValue(effectiveParams, ['endpoint']);
  const apiPath = getPreferredValue(effectiveParams, ['api_path']);
  return {
    typeLabel,
    detail:
      joinDetailSegments([
        endpoint ? `地址 ${endpoint}` : '',
        apiPath ? `路径 ${apiPath}` : '',
      ]) || '-',
  };
};

const formatSinkFileLabel = (file = '') => {
  const normalized = String(file || '').trim();
  if (!normalized) {
    return '';
  }
  const parts = normalized.split('/').filter(Boolean);
  return parts.slice(-1)[0] || normalized;
};

const isBusinessSinkFile = (file = '') => {
  const normalized = String(file || '').trim();
  return normalized.startsWith('business.d/') && !normalized.includes('/ignore/');
};

const extractWplOverviewFromContent = (content = '', fallbackPackage = '') => {
  if (!content || typeof content !== 'string') {
    return null;
  }

  const packageMatch = content.match(
    /(?:#\[tag\(([\s\S]*?)\)\]\s*)?package\s+([a-zA-Z0-9_]+)\s*\{/m,
  );
  const packageTagAttributes = parseTagAttributes(packageMatch?.[1] || '');
  const packageKey = (packageMatch?.[2] || fallbackPackage || '').trim();
  if (!packageKey || isIgnoredWplIdentifier(packageKey)) {
    return null;
  }

  const deviceType =
    packageTagAttributes.dev_type?.trim() ||
    packageTagAttributes.dev_name?.trim() ||
    packageKey;

  const logTypes = [];
  const seenRules = new Set();
  const rulePattern = /(?:#\[tag\(([\s\S]*?)\)\]\s*)?rule\s+([a-zA-Z0-9_]+)\s*\{/g;
  let ruleMatch = rulePattern.exec(content);
  while (ruleMatch) {
    const ruleKey = (ruleMatch[2] || '').trim();
    if (ruleKey && !seenRules.has(ruleKey) && !isIgnoredWplIdentifier(ruleKey)) {
      seenRules.add(ruleKey);
      const ruleTagAttributes = parseTagAttributes(ruleMatch[1] || '');
      logTypes.push({
        ruleKey,
        logTypeName: ruleTagAttributes.log_desc?.trim() || ruleKey,
      });
    }
    ruleMatch = rulePattern.exec(content);
  }

  return {
    key: packageKey,
    deviceType,
    logTypes: logTypes.sort((a, b) => a.logTypeName.localeCompare(b.logTypeName, 'zh-Hans-CN')),
  };
};

const parseTemplateDefaultValue = (value) => {
  const raw = String(value ?? '').trim();
  if (!raw) {
    return '';
  }

  if (raw.startsWith('"') && raw.endsWith('"') && raw.length >= 2) {
    return raw.slice(1, -1);
  }

  if (raw.startsWith('[') && raw.endsWith(']')) {
    return raw
      .slice(1, -1)
      .split(',')
      .map((item) => item.trim().replace(/^"|"$/g, ''))
      .filter(Boolean)
      .join(', ');
  }

  return raw;
};

const buildConnectorMetaMap = (items = []) =>
  (Array.isArray(items) ? items : []).reduce((accumulator, item) => {
    const connect = String(item?.connect || '').trim();
    if (!connect) {
      return accumulator;
    }

    const defaultParams = (Array.isArray(item?.fields) ? item.fields : []).reduce(
      (params, field) => {
        if (field?.advanced || !field?.name || field?.defaultValue === undefined || field?.defaultValue === null) {
          return params;
        }

        params[field.name] = parseTemplateDefaultValue(field.defaultValue);
        return params;
      },
      {},
    );

    accumulator[connect] = {
      typeKey: String(item?.connectorType || '').trim() || connect,
      typeLabel:
        String(item?.connectorTypeDisplayName || '').trim() ||
        String(item?.connectorType || '').trim() ||
        connect,
      defaultParams,
    };
    return accumulator;
  }, {});

const parseRuntimeDetailRows = (detail = '') =>
  String(detail || '')
    .split('，')
    .map((item) => item.trim())
    .filter(Boolean)
    .map((item) => {
      const [label, ...rest] = item.split(/\s+/);
      return {
        label: label || '',
        value: rest.join(' ').trim() || '',
      };
    })
    .filter((item) => item.label && item.value);

const renderSummaryIcon = (key) => {
  switch (key) {
    case 'device':
      return <DatabaseOutlined />;
    case 'log':
      return <FileTextOutlined />;
    case 'source':
      return <ApartmentOutlined />;
    case 'sink':
      return <FolderOpenOutlined />;
    default:
      return <NumberOutlined />;
  }
};

const renderRuntimeDetailIcon = (label) => {
  switch (label) {
    case '地址':
      return <EnvironmentOutlined />;
    case '端口':
      return <DeploymentUnitOutlined />;
    case '协议':
      return <ApartmentOutlined />;
    case '文件路径':
      return <FileTextOutlined />;
    default:
      return <ApiOutlined />;
  }
};

function IntegrationOverviewPage() {
  const { t } = useTranslation();
  const [loading, setLoading] = useState(false);
  const [rows, setRows] = useState([]);
  const [sourceItems, setSourceItems] = useState([]);
  const [sinkItems, setSinkItems] = useState([]);
  const [supportedSourceTypeCount, setSupportedSourceTypeCount] = useState(0);
  const [supportedSinkTypeCount, setSupportedSinkTypeCount] = useState(0);

  const fetchAllWplFiles = useCallback(async () => {
    const collected = [];
    const seen = new Set();
    let currentPage = 1;
    let parseFileName = '';
    let sampleFileName = '';

    while (true) {
      const result = await fetchRuleFiles({
        type: RuleType.WPL,
        page: currentPage,
        pageSize: WPL_FETCH_PAGE_SIZE,
      });
      parseFileName = result?.meta?.wplParseFile || parseFileName;
      sampleFileName = result?.meta?.wplSampleFile || sampleFileName;
      const rawItems = Array.isArray(result?.items) ? result.items : [];
      const items = normalizeWplList(rawItems, parseFileName);
      const pageSize =
        typeof result?.pageSize === 'number' && result.pageSize > 0
          ? result.pageSize
          : WPL_FETCH_PAGE_SIZE;
      const total = typeof result?.total === 'number' ? result.total : 0;
      const totalPages = total > 0 ? Math.ceil(total / pageSize) : 0;

      items.forEach((item) => {
        if (!seen.has(item)) {
          seen.add(item);
          collected.push(item);
        }
      });

      if (totalPages ? currentPage >= totalPages : !rawItems.length || rawItems.length < pageSize) {
        break;
      }
      currentPage += 1;
    }

    return {
      items: collected,
      parseFileName,
      sampleFileName,
    };
  }, []);

  const loadOverview = useCallback(async () => {
    setLoading(true);
    try {
      const [wplResult, runtimeOverview] = await Promise.all([
        fetchAllWplFiles(),
        fetchIntegrationRuntimeOverview(),
      ]);
      const files = Array.isArray(wplResult?.items) ? wplResult.items : [];
      const parseFileName = wplResult?.parseFileName || '';
      const sampleFileName = wplResult?.sampleFileName || '';

      const packageKeys = Array.from(
        new Set(
          files
            .filter((entry) => !isWplSampleEntry(entry, parseFileName, sampleFileName))
            .map((entry) => getWplEntryParts(entry, parseFileName).rule)
            .filter(Boolean),
        ),
      );

      const overviewResults = await Promise.all(
        packageKeys.map(async (packageKey) => {
          const response = await fetchRuleConfig({
            type: RuleType.WPL,
            file: `${packageKey}/${parseFileName}`,
          });
          return extractWplOverviewFromContent(response?.content || '', packageKey);
        }),
      );

      const nextRows = overviewResults
        .filter(Boolean)
        .sort((a, b) => a.deviceType.localeCompare(b.deviceType, 'zh-Hans-CN'));

      const logNameCount = nextRows.reduce((accumulator, item) => {
        item.logTypes.forEach((logType) => {
          const current = accumulator.get(logType.logTypeName) || 0;
          accumulator.set(logType.logTypeName, current + 1);
        });
        return accumulator;
      }, new Map());

      setRows(
        nextRows.map((item) => ({
          ...item,
          logTypes: item.logTypes.map((logType) => ({
            ...logType,
            showRuleKey:
              (logNameCount.get(logType.logTypeName) || 0) > 1 &&
              logType.logTypeName !== logType.ruleKey,
          })),
        })),
      );
      setSourceItems(Array.isArray(runtimeOverview?.sources) ? runtimeOverview.sources : []);
      setSinkItems(Array.isArray(runtimeOverview?.sinks) ? runtimeOverview.sinks : []);
      setSupportedSourceTypeCount(runtimeOverview?.supportedSourceTypeCount || 0);
      setSupportedSinkTypeCount(runtimeOverview?.supportedSinkTypeCount || 0);
    } catch (error) {
      message.error(t('integrationOverview.loadFailed', { message: error.message }));
    } finally {
      setLoading(false);
    }
  }, [fetchAllWplFiles, t]);

  useEffect(() => {
    loadOverview();
  }, [loadOverview]);

  const summary = useMemo(
    () => ({
      deviceTypeCount: rows.length,
      logTypeCount: rows.reduce((total, item) => total + item.logTypes.length, 0),
    }),
    [rows],
  );

  const columns = [
    {
      title: '序号',
      key: 'index',
      width: 80,
      render: (_, __, index) => index + 1,
    },
    {
      title: t('integrationOverview.deviceType'),
      dataIndex: 'deviceType',
      key: 'deviceType',
      width: 180,
      render: (deviceType) => (
        <Tag className="integration-overview-device-tag" bordered={false}>
          {deviceType}
        </Tag>
      ),
    },
    {
      title: t('integrationOverview.logTypeCount'),
      dataIndex: 'logTypes',
      key: 'logTypeCount',
      width: 120,
      render: (logTypes) => logTypes.length,
    },
    {
      title: t('integrationOverview.logStatus'),
      dataIndex: 'logTypes',
      key: 'logStatus',
      width: 320,
      render: (logTypes) => (
        <div className="integration-overview-log-list">
          {logTypes.length > 0 ? (
            logTypes.map((logType) => (
              <div key={logType.ruleKey} className="integration-overview-log-item">
                <span className="integration-overview-log-primary">{logType.logTypeName}</span>
                {logType.showRuleKey ? (
                  <span className="integration-overview-log-secondary">{logType.ruleKey}</span>
                ) : null}
              </div>
            ))
          ) : (
            <div className="integration-overview-log-item integration-overview-log-item--empty">
              -
            </div>
          )}
        </div>
      ),
    },
  ];

  const renderRuntimeSection = (title, items, emptyKey, options = {}) => (
    <section className="integration-overview-runtime-section">
      <div className="integration-overview-runtime-header">
        <h3>{title}</h3>
        <span className="integration-overview-runtime-count">{items.length}</span>
      </div>
      <div className="integration-overview-runtime-list">
        {items.length > 0 ? (
          items.map((item, index) => (
            <article key={item.key} className="integration-overview-runtime-card">
              <div className="integration-overview-runtime-card-main">
                <span className="integration-overview-runtime-index">
                  {String(index + 1).padStart(2, '0')}
                </span>
                <div className="integration-overview-runtime-card-body">
                  <div className="integration-overview-runtime-card-head">
                    <strong>{item.title}</strong>
                    <span className="integration-overview-runtime-type">{item.typeLabel}</span>
                  </div>
                  {options.showMeta && 'metaLabel' in item && item.metaLabel ? (
                    <div className="integration-overview-runtime-meta">{item.metaLabel}</div>
                  ) : null}
                  <div className="integration-overview-runtime-detail">
                    {parseRuntimeDetailRows(item.detail).length > 0 ? (
                      parseRuntimeDetailRows(item.detail).map((detailItem) => (
                        <div
                          key={`${item.key}-${detailItem.label}`}
                          className="integration-overview-runtime-detail-row"
                        >
                          <span className="integration-overview-runtime-detail-icon">
                            {renderRuntimeDetailIcon(detailItem.label)}
                          </span>
                          <span className="integration-overview-runtime-detail-label">
                            {detailItem.label}
                          </span>
                          <span className="integration-overview-runtime-detail-value">
                            {detailItem.value}
                          </span>
                        </div>
                      ))
                    ) : (
                      <div className="integration-overview-runtime-detail-empty">-</div>
                    )}
                  </div>
                </div>
              </div>
            </article>
          ))
        ) : (
          <div className="integration-overview-runtime-empty">
            {t(`integrationOverview.${emptyKey}`)}
          </div>
        )}
      </div>
    </section>
  );

  const summaryCards = [
    {
      key: 'device',
      label: t('integrationOverview.coveredDeviceTypes'),
      value: summary.deviceTypeCount,
    },
    {
      key: 'log',
      label: t('integrationOverview.coveredLogTypes'),
      value: summary.logTypeCount,
    },
    {
      key: 'source',
      label: t('integrationOverview.supportedSourceTypes'),
      value: supportedSourceTypeCount,
    },
    {
      key: 'sink',
      label: t('integrationOverview.supportedSinkTypes'),
      value: supportedSinkTypeCount,
    },
  ];

  return (
    <section className="page-panels integration-overview-page">
      <article className="panel is-visible integration-overview-panel">
        <header className="panel-header">
          <h2>{t('integrationOverview.title')}</h2>
        </header>
        <section className="panel-body integration-overview-body">
          <div className="integration-overview-toolbar">
            <div className="integration-overview-summary">
              {summaryCards.map((card) => (
                <div key={card.key} className="integration-overview-summary-card">
                  <span className="integration-overview-summary-icon">
                    {renderSummaryIcon(card.key)}
                  </span>
                  <div className="integration-overview-summary-content">
                    <span className="integration-overview-summary-label">{card.label}</span>
                    <strong className="integration-overview-summary-value">{card.value}</strong>
                  </div>
                </div>
              ))}
            </div>
          </div>

          <div className="integration-overview-runtime-grid">
            {renderRuntimeSection(
              t('integrationOverview.sourceOverview'),
              sourceItems,
              'sourceEmpty',
              { showMeta: false },
            )}
            {renderRuntimeSection(
              t('integrationOverview.sinkOverview'),
              sinkItems,
              'sinkEmpty',
              { showMeta: false },
            )}
          </div>

          <Table
            className="release-table integration-overview-table"
            rowKey="key"
            loading={loading}
            columns={columns}
            dataSource={rows}
            scroll={{ x: 1080 }}
            pagination={{
              pageSize: 10,
              showSizeChanger: false,
              hideOnSinglePage: true,
            }}
            locale={{
              emptyText: t('integrationOverview.empty'),
            }}
          />
        </section>
      </article>
    </section>
  );
}

export default IntegrationOverviewPage;
