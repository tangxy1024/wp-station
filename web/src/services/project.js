import httpRequest from './request';

const API_BASE = '/api';

function getOperatorHeader() {
  const username = sessionStorage.getItem('username');
  return username ? { 'X-Operator': encodeURIComponent(username) } : {};
}

async function parseErrorResponse(response, fallback) {
  try {
    const body = await response.json();
    return body?.error?.message || body?.error?.details || fallback;
  } catch (_) {
    return fallback;
  }
}

export async function importProjectArchive(file) {
  const response = await fetch(`${API_BASE}/project/import/archive`, {
    method: 'POST',
    headers: {
      ...getOperatorHeader(),
      'Content-Type': 'application/octet-stream',
      'X-File-Name': encodeURIComponent(file.name),
    },
    body: file,
  });

  if (!response.ok) {
    throw new Error(await parseErrorResponse(response, '导入配置包失败'));
  }

  return response.json();
}

export async function confirmProjectArchiveImport(importId) {
  const response = await fetch(`${API_BASE}/project/import/archive/confirm`, {
    method: 'POST',
    headers: {
      ...getOperatorHeader(),
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ import_id: importId }),
  });

  if (!response.ok) {
    throw new Error(await parseErrorResponse(response, '确认导入配置包失败'));
  }

  return response.json();
}

export async function importProjectFromFiles({ sourceDir }) {
  const response = await httpRequest.post('/project/import', {
    source_dir: sourceDir,
  });
  return response?.summary ? response : response?.data || response;
}

export async function exportProjectArchive() {
  const response = await fetch(`${API_BASE}/project/export/archive`, {
    method: 'GET',
    headers: getOperatorHeader(),
  });

  if (!response.ok) {
    throw new Error(await parseErrorResponse(response, '导出配置包失败'));
  }

  const blob = await response.blob();
  const disposition = response.headers.get('content-disposition') || '';
  const match = disposition.match(/filename="?([^";]+)"?/i);
  const fileName = match?.[1] || `wp-station-project-${Date.now()}.tar.gz`;
  return { blob, fileName };
}

export function downloadBlob(blob, fileName) {
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = fileName;
  document.body.appendChild(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
}
