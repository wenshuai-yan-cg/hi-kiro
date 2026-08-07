declare module 'react-calendar-heatmap' {
  import React from 'react';
  interface HeatmapValue { date: string; count?: number; [key: string]: unknown; }
  interface Props {
    startDate: Date;
    endDate: Date;
    values: HeatmapValue[];
    classForValue?: (value: HeatmapValue | null) => string;
    titleForValue?: (value: HeatmapValue | null) => string;
    onClick?: (value: HeatmapValue | null) => void;
    [key: string]: unknown;
  }
  const CalendarHeatmap: React.FC<Props>;
  export default CalendarHeatmap;
}
