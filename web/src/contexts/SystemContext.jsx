/**
 * 前端双系统全局 Context。
 *
 * 负责三件事：
 * 1. 管理当前系统 `wparse / wfusion`
 * 2. 同步 URL query 与 localStorage
 * 3. 提供切换中的轻量状态，供导航和页面做局部反馈
 */

import React, { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import {
  AVAILABLE_SYSTEMS,
  buildSystemSearch,
  ensureSystemInitialized,
  getSharedSystem,
  normalizeSystem,
  setCurrentSystem as persistCurrentSystem,
  withSystemPath,
} from '@/services/system';

const SWITCH_FEEDBACK_MS = 260;
const SystemContext = createContext(null);

function hasSystemParam(search = '') {
  const params = new URLSearchParams(search || '');
  return params.has('system');
}

export function SystemProvider({ children }) {
  const location = useLocation();
  const navigate = useNavigate();
  const [currentSystem, setCurrentSystem] = useState(() => {
    const searchParams = new URLSearchParams(location.search || '');
    return ensureSystemInitialized(searchParams.get('system'));
  });
  const [isSwitchingSystem, setIsSwitchingSystem] = useState(false);
  const switchTimerRef = useRef(null);
  const beforeSwitchHandlersRef = useRef(new Set());

  useEffect(() => {
    const params = new URLSearchParams(location.search || '');
    const querySystem = params.get('system');
    const nextSystem = normalizeSystem(querySystem || currentSystem);

    if (querySystem && nextSystem !== currentSystem) {
      persistCurrentSystem(nextSystem);
      setCurrentSystem(nextSystem);
      return;
    }

    if (!querySystem) {
      const nextSearch = buildSystemSearch(location.search, currentSystem);
      navigate(
        {
          pathname: location.pathname,
          search: nextSearch ? `?${nextSearch}` : '',
        },
        { replace: true },
      );
    }
  }, [currentSystem, location.pathname, location.search, navigate]);

  useEffect(() => {
    return () => {
      if (switchTimerRef.current) {
        window.clearTimeout(switchTimerRef.current);
      }
    };
  }, []);

  const registerBeforeSystemSwitch = useCallback((handler) => {
    if (typeof handler !== 'function') {
      return () => {};
    }

    beforeSwitchHandlersRef.current.add(handler);
    return () => {
      beforeSwitchHandlersRef.current.delete(handler);
    };
  }, []);

  const canSwitchSystem = useCallback(
    async (nextSystem) => {
      for (const handler of beforeSwitchHandlersRef.current) {
        const result = await handler(nextSystem, currentSystem);
        if (result === false) {
          return false;
        }
      }

      return true;
    },
    [currentSystem],
  );

  const switchSystem = async (system) => {
    const nextSystem = normalizeSystem(system);
    if (nextSystem === currentSystem) {
      return true;
    }

    const allowed = await canSwitchSystem(nextSystem);
    if (!allowed) {
      return false;
    }

    setIsSwitchingSystem(true);
    persistCurrentSystem(nextSystem);
    setCurrentSystem(nextSystem);

    const nextSearch = buildSystemSearch(location.search, nextSystem);
    navigate(
      {
        pathname: location.pathname,
        search: nextSearch ? `?${nextSearch}` : '',
      },
      { replace: true },
    );

    if (switchTimerRef.current) {
      window.clearTimeout(switchTimerRef.current);
    }
    switchTimerRef.current = window.setTimeout(() => {
      setIsSwitchingSystem(false);
      switchTimerRef.current = null;
    }, SWITCH_FEEDBACK_MS);

    return true;
  };

  const decoratePath = (path) => withSystemPath(path, currentSystem);

  const value = useMemo(
    () => ({
      currentSystem,
      isSwitchingSystem,
      switchSystem,
      registerBeforeSystemSwitch,
      availableSystems: AVAILABLE_SYSTEMS,
      sharedSystem: getSharedSystem(),
      decoratePath,
      hasSystemInUrl: hasSystemParam(location.search),
    }),
    [currentSystem, decoratePath, isSwitchingSystem, location.search, registerBeforeSystemSwitch],
  );

  return <SystemContext.Provider value={value}>{children}</SystemContext.Provider>;
}

export function useSystem() {
  const context = useContext(SystemContext);
  if (!context) {
    throw new Error('useSystem 必须在 SystemProvider 内使用');
  }
  return context;
}
