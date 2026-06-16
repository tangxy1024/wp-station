import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Input, Table, message } from 'antd';
import { fetchConnectionFiles, fetchRuleConfig, fetchRuleFiles, RuleType } from '@/services/config';

const WPL_FETCH_PAGE_SIZE = 50;
const WPL_PARSE_FILE = 'parse.wpl';
const WPL_SAMPLE_FILE = 'sample.dat';
const WPSRC_FILE = 'wpsrc.toml';

const SOURCE_CONNECTOR_EXACT_META = Object.freeze({
  file_src: {
    label: 'file',
    defaultParams: {
      base: 'data/in_dat',
      file: 'gen.dat',
    },
  },
  syslog_udp_src: {
    label: 'syslog-udp',
    defaultParams: {
      addr: '0.0.0.0',
      port: '1514',
    },
  },
  syslog_tcp_src: {
    label: 'syslog-tcp',
    defaultParams: {
      addr: '127.0.0.1',
      port: '1514',
    },
  },
  tcp_src: {
    label: 'tcp',
    defaultParams: {
      addr: '0.0.0.0',
      port: '9000',
    },
  },
  mysql_src: {
    label: 'mysql',
  },
  postgres_src: {
    label: 'postgres',
  },
  kafka_src: {
    label: 'kafka',
  },
  dmdb_connect_string_src: {
    label: 'dmdb',
  },
});

const SOURCE_CONNECTOR_FALLBACK_META = [
  { pattern: /syslog[_-]?udp/i, label: 'syslog-udp' },
  { pattern: /syslog[_-]?tcp/i, label: 'syslog-tcp' },
  { pattern: /tcp/i, label: 'tcp' },
  { pattern: /udp/i, label: 'udp' },
  { pattern: /mysql/i, label: 'mysql' },
  { pattern: /postgres/i, label: 'postgres' },
  { pattern: /doris/i, label: 'doris' },
  { pattern: /clickhouse/i, label: 'clickhouse' },
  { pattern: /dmdb[_-]?connect[_-]?string/i, label: 'dmdb' },
  { pattern: /dmdb/i, label: 'dmdb' },
  { pattern: /file/i, label: 'file' },
  { pattern: /kafka/i, label: 'kafka' },
];

const SINK_CONNECTOR_EXACT_META = Object.freeze({
  file_proto_sink: {
    label: 'file',
    defaultParams: {
      base: './data/out_dat',
      file: 'default.dat',
    },
  },
  file_json_sink: {
    label: 'file',
    defaultParams: {
      base: './data/out_dat',
      file: 'default.json',
    },
  },
  file_kv_sink: {
    label: 'file',
    defaultParams: {
      base: './data/out_dat',
      file: 'default.kv',
    },
  },
  file_raw_sink: {
    label: 'file',
    defaultParams: {
      base: './data/out_dat',
      file: 'default.raw',
    },
  },
  syslog_udp_sink: {
    label: 'syslog-udp',
    defaultParams: {
      addr: '127.0.0.1',
      port: '1514',
    },
  },
  syslog_tcp_sink: {
    label: 'syslog-tcp',
    defaultParams: {
      addr: '127.0.0.1',
      port: '1514',
    },
  },
  tcp_sink: {
    label: 'tcp',
    defaultParams: {
      addr: '127.0.0.1',
      port: '9000',
    },
  },
  udp_out_sink: {
    label: 'udp',
    defaultParams: {
      addr: '127.0.0.1',
      port: '31600',
    },
  },
  kafka_sink: {
    label: 'kafka',
  },
  mysql_sink: {
    label: 'mysql',
  },
  postgres_sink: {
    label: 'postgres',
  },
  doris_sink: {
    label: 'doris',
  },
  clickhouse_sink: {
    label: 'clickhouse',
  },
  elasticsearch_sink: {
    label: 'elasticsearch',
  },
  victorialogs_sink: {
    label: 'victorialogs',
  },
  victoriametrics_sink: {
    label: 'victoriametrics',
  },
  prometheus_sink: {
    label: 'prometheus',
  },
  http_sink: {
    label: 'http',
  },
  dmdb_connect_string_sink: {
    label: 'dmdb',
  },
});

const SINK_CONNECTOR_FALLBACK_META = [
  { pattern: /syslog[_-]?udp/i, label: 'syslog-udp' },
  { pattern: /syslog[_-]?tcp/i, label: 'syslog-tcp' },
  { pattern: /tcp/i, label: 'tcp' },
  { pattern: /udp/i, label: 'udp' },
  { pattern: /mysql/i, label: 'mysql' },
  { pattern: /postgres/i, label: 'postgres' },
  { pattern: /doris/i, label: 'doris' },
  { pattern: /clickhouse/i, label: 'clickhouse' },
  { pattern: /elasticsearch/i, label: 'elasticsearch' },
  { pattern: /prometheus/i, label: 'prometheus' },
  { pattern: /victorialogs/i, label: 'victorialogs' },
  { pattern: /victoriametrics/i, label: 'victoriametrics' },
  { pattern: /http/i, label: 'http' },
  { pattern: /dmdb[_-]?connect[_-]?string/i, label: 'dmdb' },
  { pattern: /dmdb/i, label: 'dmdb' },
  { pattern: /kafka/i, label: 'kafka' },
  { pattern: /file/i, label: 'file' },
];

const CONNECTOR_TYPE_DISPLAY_NAMES = Object.freeze({
  file: '文件',
  kafka: 'Kafka',
  dmdb: '达梦数据库',
  mysql: 'MySQL',
  postgres: 'PostgreSQL',
  doris: 'Doris',
  clickhouse: 'ClickHouse',
  elasticsearch: 'Elasticsearch',
  victorialogs: 'VictoriaLogs',
  victoriametrics: 'VictoriaMetrics',
  prometheus: 'Prometheus',
  http: 'HTTP',
  'syslog-udp': 'Syslog UDP',
  'syslog-tcp': 'Syslog TCP',
  tcp: 'TCP',
  udp: 'UDP',
});

const normalizeWplEntry = (value) => {
  if (value === undefined || value === null) {
    return '';
  }
  const trimmed = String(value).trim();
  if (!trimmed) {
    return '';
  }
  if (!trimmed.includes('/')) {
    return `${trimmed}/${WPL_PARSE_FILE}`;
  }
  const [rulePart, ...restParts] = trimmed.split('/');
  const rule = (rulePart || '').trim();
  const sub = (restParts.join('/') || '').trim() || WPL_PARSE_FILE;
  if (!rule) {
    return sub;
  }
  return `${rule}/${sub}`;
};

const normalizeWplList = (items) => {
  const deduped = new Set();
  (Array.isArray(items) ? items : []).forEach((item) => {
    const entry = normalizeWplEntry(item);
    if (entry) {
      deduped.add(entry);
    }
  });
  return Array.from(deduped);
};

const getWplEntryParts = (entry) => {
  const normalized = normalizeWplEntry(entry);
  if (!normalized) {
    return { rule: '', sub: '' };
  }
  const [rule, sub] = normalized.split('/');
  return {
    rule: (rule || '').trim(),
    sub: (sub || '').trim(),
  };
};

const isWplSampleEntry = (entry) => normalizeWplEntry(entry).endsWith(`/${WPL_SAMPLE_FILE}`);
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

const resolveConnectorMeta = (connect = '', scope = 'source') => {
  const normalized = String(connect || '').trim();
  if (!normalized) {
    return {
      label: '-',
      defaultParams: {},
    };
  }

  const exactMeta =
    scope === 'sink'
      ? SINK_CONNECTOR_EXACT_META[normalized]
      : SOURCE_CONNECTOR_EXACT_META[normalized];
  if (exactMeta) {
    return exactMeta;
  }

  const fallbackMapping =
    scope === 'sink' ? SINK_CONNECTOR_FALLBACK_META : SOURCE_CONNECTOR_FALLBACK_META;
  const matched = fallbackMapping.find((item) => item.pattern.test(normalized));
  if (matched) {
    return {
      ...matched,
      defaultParams: {},
    };
  }

  return {
    label: normalized.replace(/_(src|sink)$/i, '').replace(/_/g, '-'),
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

const buildEffectiveParams = (connect = '', params = {}, scope = 'source') => {
  const meta = resolveConnectorMeta(connect, scope);
  const merged = {
    ...(meta.defaultParams || {}),
    ...(params || {}),
  };

  if (merged.base === true) {
    merged.base = meta.defaultParams?.base || '';
  }

  return merged;
};

const buildConnectorDetail = (connect = '', params = {}, scope = 'source') => {
  const meta = resolveConnectorMeta(connect, scope);
  const typeKey = meta.label;
  const effectiveParams = buildEffectiveParams(connect, params, scope);
  const typeLabel = CONNECTOR_TYPE_DISPLAY_NAMES[typeKey] || typeKey;

  if (['syslog-udp', 'syslog-tcp', 'tcp', 'udp'].includes(typeKey)) {
    const addr = getPreferredValue(effectiveParams, ['addr', 'host']);
    const port = getPreferredValue(effectiveParams, ['port']);
    return {
      typeLabel,
      detail:
        joinDetailSegments([
          addr ? `地址 ${addr}` : '',
          port ? `端口 ${port}` : '',
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
    const normalizedPath = base && file ? `${base}/${file}` : base || file;
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

function IntegrationOverviewPage() {
  const { t } = useTranslation();
  const [loading, setLoading] = useState(false);
  const [keyword, setKeyword] = useState('');
  const [rows, setRows] = useState([]);
  const [sourceItems, setSourceItems] = useState([]);
  const [sinkItems, setSinkItems] = useState([]);
  const [supportedSourceTypeCount, setSupportedSourceTypeCount] = useState(0);
  const [supportedSinkTypeCount, setSupportedSinkTypeCount] = useState(0);

  const fetchAllWplFiles = useCallback(async () => {
    const collected = [];
    const seen = new Set();
    let currentPage = 1;

    while (true) {
      const result = await fetchRuleFiles({
        type: RuleType.WPL,
        page: currentPage,
        pageSize: WPL_FETCH_PAGE_SIZE,
      });
      const rawItems = Array.isArray(result?.items) ? result.items : [];
      const items = normalizeWplList(rawItems);
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

    return collected;
  }, []);

  const loadOverview = useCallback(async () => {
    setLoading(true);
    try {
      const [files, sourceConfig, sinkFilesResponse, connectionFiles] = await Promise.all([
        fetchAllWplFiles(),
        fetchRuleConfig({ type: RuleType.SOURCE, file: WPSRC_FILE }),
        fetchRuleFiles({ type: RuleType.SINK }),
        fetchConnectionFiles(),
      ]);

      const packageKeys = Array.from(
        new Set(
          files
            .filter((entry) => !isWplSampleEntry(entry))
            .map((entry) => getWplEntryParts(entry).rule)
            .filter(Boolean),
        ),
      );

      const overviewResults = await Promise.all(
        packageKeys.map(async (packageKey) => {
          const response = await fetchRuleConfig({
            type: RuleType.WPL,
            file: `${packageKey}/${WPL_PARSE_FILE}`,
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

      const parsedSources = parseTomlBlocks(
        sourceConfig?.content || '',
        '[[sources]]',
        '[sources.params]',
      )
        .filter((item) => item.enable === true)
        .map((item) => {
          const summary = buildConnectorDetail(item.connect, item.params || {}, 'source');
          return {
            key: item.key || item.connect,
            connect: item.connect || '',
            title: item.key || item.connect || '-',
            typeLabel: summary.typeLabel,
            detail: summary.detail,
          };
        })
        .sort((a, b) => a.title.localeCompare(b.title, 'zh-Hans-CN'));
      setSourceItems(parsedSources);

      const sinkFileItems = Array.isArray(sinkFilesResponse?.items) ? sinkFilesResponse.items : [];
      const sinkConfigs = await Promise.all(
        sinkFileItems
          .map((item) => (typeof item === 'string' ? item : item?.file))
          .filter((file) => isBusinessSinkFile(file))
          .map(async (file) => {
            if (!file) {
              return [];
            }
            const response = await fetchRuleConfig({
              type: RuleType.SINK,
              file,
            });
            const sinks = parseTomlBlocks(
              response?.content || '',
              '[[sink_group.sinks]]',
              '[sink_group.sinks.params]',
            );

            return sinks.map((sink, index) => {
              const summary = buildConnectorDetail(sink.connect, sink.params || {}, 'sink');
              const fileLabel = formatSinkFileLabel(file);
              return {
                key: `${file}-${sink.name || sink.connect || index}`,
                file,
                title: fileLabel,
                typeLabel: summary.typeLabel,
                detail: summary.detail,
              };
            });
          }),
      );

      setSinkItems(
        sinkConfigs
          .flat()
          .filter(Boolean)
          .sort((a, b) => a.title.localeCompare(b.title, 'zh-Hans-CN')),
      );

      setSupportedSourceTypeCount(Array.isArray(connectionFiles?.sources) ? connectionFiles.sources.length : 0);
      setSupportedSinkTypeCount(Array.isArray(connectionFiles?.sinks) ? connectionFiles.sinks.length : 0);
    } catch (error) {
      message.error(t('integrationOverview.loadFailed', { message: error.message }));
    } finally {
      setLoading(false);
    }
  }, [fetchAllWplFiles, t]);

  useEffect(() => {
    loadOverview();
  }, [loadOverview]);

  const filteredRows = useMemo(() => {
    const normalizedKeyword = keyword.trim().toLowerCase();
    if (!normalizedKeyword) {
      return rows;
    }

    return rows.filter((item) => {
      if (item.deviceType.toLowerCase().includes(normalizedKeyword)) {
        return true;
      }
      return item.logTypes.some(
        (logType) =>
          logType.logTypeName.toLowerCase().includes(normalizedKeyword) ||
          logType.ruleKey.toLowerCase().includes(normalizedKeyword),
      );
    });
  }, [keyword, rows]);

  const filteredSourceItems = useMemo(() => {
    const normalizedKeyword = keyword.trim().toLowerCase();
    if (!normalizedKeyword) {
      return sourceItems;
    }
    return sourceItems.filter(
      (item) =>
        item.title.toLowerCase().includes(normalizedKeyword) ||
        item.typeLabel.toLowerCase().includes(normalizedKeyword) ||
        String(item.detail || '').toLowerCase().includes(normalizedKeyword),
    );
  }, [keyword, sourceItems]);

  const filteredSinkItems = useMemo(() => {
    const normalizedKeyword = keyword.trim().toLowerCase();
    if (!normalizedKeyword) {
      return sinkItems;
    }
    return sinkItems.filter(
      (item) =>
        item.title.toLowerCase().includes(normalizedKeyword) ||
        item.typeLabel.toLowerCase().includes(normalizedKeyword) ||
        String(item.detail || '').toLowerCase().includes(normalizedKeyword),
    );
  }, [keyword, sinkItems]);

  const summary = useMemo(
    () => ({
      deviceTypeCount: filteredRows.length,
      logTypeCount: filteredRows.reduce((total, item) => total + item.logTypes.length, 0),
    }),
    [filteredRows],
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
      width: '24%',
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
                  <div className="integration-overview-runtime-detail">{item.detail || '-'}</div>
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

  return (
    <section className="page-panels">
      <article className="panel is-visible">
        <header className="panel-header">
          <h2>{t('integrationOverview.title')}</h2>
        </header>
        <section className="panel-body integration-overview-body">
          <div className="integration-overview-toolbar">
            <div className="integration-overview-summary">
              <div className="integration-overview-summary-card">
                <span className="integration-overview-summary-label">
                  {t('integrationOverview.coveredDeviceTypes')}
                </span>
                <strong className="integration-overview-summary-value">
                  {summary.deviceTypeCount}
                </strong>
              </div>
              <div className="integration-overview-summary-card">
                <span className="integration-overview-summary-label">
                  {t('integrationOverview.coveredLogTypes')}
                </span>
                <strong className="integration-overview-summary-value">
                  {summary.logTypeCount}
                </strong>
              </div>
              <div className="integration-overview-summary-card">
                <span className="integration-overview-summary-label">
                  {t('integrationOverview.supportedSourceTypes')}
                </span>
                <strong className="integration-overview-summary-value">
                  {supportedSourceTypeCount}
                </strong>
              </div>
              <div className="integration-overview-summary-card">
                <span className="integration-overview-summary-label">
                  {t('integrationOverview.supportedSinkTypes')}
                </span>
                <strong className="integration-overview-summary-value">
                  {supportedSinkTypeCount}
                </strong>
              </div>
            </div>
            <div className="integration-overview-filters">
              <Input
                allowClear
                value={keyword}
                onChange={(event) => setKeyword(event.target.value)}
                placeholder={t('integrationOverview.searchPlaceholder')}
              />
            </div>
          </div>

          <div className="integration-overview-runtime-grid">
            {renderRuntimeSection(
              t('integrationOverview.sourceOverview'),
              filteredSourceItems,
              'sourceEmpty',
              { showMeta: false },
            )}
            {renderRuntimeSection(
              t('integrationOverview.sinkOverview'),
              filteredSinkItems,
              'sinkEmpty',
              { showMeta: false },
            )}
          </div>

          <Table
            className="release-table integration-overview-table"
            rowKey="key"
            loading={loading}
            columns={columns}
            dataSource={filteredRows}
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
