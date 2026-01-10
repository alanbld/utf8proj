#!/usr/bin/env python3
"""
MS Project to utf8proj Companion Tool
Converts .mpp files to Project 2010 XML (MSPDI) or utf8proj (.proj) format.

Dependencies:
- mpxj (MS Project Java library wrapper)
- jpype1 (Python-Java bridge)

This is a concise version optimized for the utf8proj ecosystem.
"""

import mpxj
import jpype
import jpype.imports
import os
import sys
import re
from pathlib import Path
from datetime import datetime

class UTF8ProjWriter:
    def __init__(self, project):
        self.project = project
        self.lines = []
        self.resource_map = {} # Map MPXJ resource UniqueID to utf8proj ID

    def sanitize_id(self, original_id):
        """Convert an ID to a safe utf8proj identifier."""
        safe_id = str(original_id).lower()
        safe_id = re.sub(r'[^a-z0-9_]', '_', safe_id)
        safe_id = re.sub(r'_+', '_', safe_id).strip('_')
        if not safe_id:
            safe_id = "res_" + str(hash(original_id))
        return safe_id

    def sanitize_task_id(self, task_id):
        return f"task_{task_id}"

    def format_date(self, java_date):
        """Convert Java Date/LocalDateTime to YYYY-MM-DD."""
        if not java_date:
            return None
        try:
            return str(java_date)[:10]
        except:
            return None

    def add_line(self, text, indent=0):
        self.lines.append("    " * indent + text)

    def write(self, output_file):
        self.generate_project_block()
        self.generate_calendar()
        self.generate_resources()
        self.generate_tasks()
        with open(output_file, 'w', encoding='utf-8') as f:
            f.write('\n'.join(self.lines))

    def generate_calendar(self):
        year = datetime.now().year
        self.add_line(f'calendar "standard" {{')
        self.add_line('working_days: mon-fri', 1)
        self.add_line('working_hours: 09:00-18:00', 1)
        holidays = [
            ("New Year", f"{year}-01-01"),
            ("Epiphany", f"{year}-01-06"),
            ("Liberation Day", f"{year}-04-25"),
            ("Labor Day", f"{year}-05-01"),
            ("Republic Day", f"{year}-06-02"),
            ("Assumption", f"{year}-08-15"),
            ("All Saints", f"{year}-11-01"),
            ("Immaculate Conception", f"{year}-12-08"),
            ("Christmas", f"{year}-12-25"),
            ("St. Stephen", f"{year}-12-26"),
        ]
        for name, date in holidays:
            self.add_line(f'holiday "{name}" {date}', 1)
        self.add_line('}')
        self.add_line('')

    def generate_project_block(self):
        props = self.project.getProjectProperties()
        title = props.getProjectTitle() or "Untitled"
        start_date = self.format_date(props.getStartDate()) or datetime.now().strftime("%Y-%m-%d")
        finish_date = self.format_date(props.getFinishDate())
        self.add_line(f'project "{title}" {{')
        self.add_line(f'start: {start_date}', 1)
        if finish_date:
            self.add_line(f'end: {finish_date}', 1)
        self.add_line(f'currency: EUR', 1)
        self.add_line('}')
        self.add_line('')

    def generate_resources(self):
        resources = self.project.getResources()
        for res in resources:
            if res.getUniqueID() is None: continue
            name = res.getName()
            if not name: continue
            tj_id = self.sanitize_id(name)
            base_id = tj_id
            counter = 1
            while tj_id in self.resource_map.values():
                tj_id = f"{base_id}_{counter}"
                counter += 1
            self.resource_map[res.getUniqueID()] = tj_id
            self.add_line(f'resource {tj_id} "{name}" {{')
            self.add_line('}', 0)
        self.add_line('')

    def generate_tasks(self):
        # MPXJ structure: Root task is usually ID 0
        top_tasks = self.project.getChildTasks()
        if top_tasks.size() == 1 and top_tasks.get(0).getID() == 0:
            top_tasks = top_tasks.get(0).getChildTasks()
        for task in top_tasks:
            self.write_task_recursive(task, indent=0)

    def get_full_id(self, task):
        parts = []
        current = task
        while current is not None and current.getUniqueID() is not None:
            if current.getID() == 0: break
            parts.insert(0, self.sanitize_task_id(current.getUniqueID()))
            current = current.getParentTask()
        return '.'.join(parts)

    def write_task_recursive(self, task, indent):
        if task.getUniqueID() is None: return

        task_id = self.sanitize_task_id(task.getUniqueID())
        original_name = str(task.getName()) if task.getName() else f"Task {task.getUniqueID()}"
        wbs = str(task.getWBS()) if task.getWBS() else str(task.getUniqueID())
        name = f"[{wbs}] {original_name}"
        name = name.replace('"', '\\"')
        self.add_line(f'task {task_id} "{name}" {{', indent)
        
        children = task.getChildTasks()
        is_container = children is not None and not children.isEmpty()
        
        if not is_container:
            if task.getMilestone():
                self.add_line('milestone: true', indent + 1)
            else:
                duration = task.getDuration()
                if duration:
                    d_val = float(duration.getDuration())
                    if d_val > 0:
                        val_str = str(int(d_val)) if d_val.is_integer() else str(d_val)
                        self.add_line(f'duration: {val_str}d', indent + 1)
                
                work = task.getWork()
                if work:
                    w_val = float(work.getDuration())
                    # Convert to days based on unit (MS Project stores Work in hours by default)
                    units = str(work.getUnits()).upper() if work.getUnits() else ""
                    if units in ("H", "HOURS", "HOUR"):
                        w_val = w_val / 8.0  # 8 hours per day
                    elif units in ("M", "MINUTES", "MINUTE"):
                        w_val = w_val / 480.0  # 8 hours * 60 minutes per day
                    # D, DAYS, W, WEEKS, MO, MONTHS assumed already in days or handled by MPXJ
                    if w_val > 0:
                        val_str = str(int(w_val)) if w_val == int(w_val) else f"{w_val:.1f}"
                        self.add_line(f'effort: {val_str}d', indent + 1)
            
        predecessors = task.getPredecessors()
        if predecessors:
            dep_strs = []
            for rel in predecessors:
                target = rel.getPredecessorTask()
                if target:
                    target_full_id = self.get_full_id(target)
                    rel_type = str(rel.getType()).upper()
                    suffix = ""
                    # MPXJ returns "SS", "FF", "SF", "FS" (or older "START_START" etc.)
                    if rel_type in ("SS", "START_START"): suffix = " SS"
                    elif rel_type in ("FF", "FINISH_FINISH"): suffix = " FF"
                    elif rel_type in ("SF", "START_FINISH"): suffix = " SF"
                    # FS (FINISH_START) is the default, no suffix needed
                    
                    lag = rel.getLag()
                    lag_str = ""
                    if lag and lag.getDuration() != 0:
                        val = float(lag.getDuration())
                        val_str = str(int(val)) if val.is_integer() else str(val)
                        sign = "+" if val > 0 else ""
                        lag_str = f" {sign}{val_str}d"
                    dep_strs.append(f"{target_full_id}{suffix}{lag_str}")
            if dep_strs:
                self.add_line(f'depends: {", ".join(dep_strs)}', indent + 1)
        
        assignments = task.getResourceAssignments()
        if assignments:
            res_ids = [self.resource_map[a.getResource().getUniqueID()] 
                       for a in assignments if a.getResource() and a.getResource().getUniqueID() in self.resource_map]
            if res_ids:
                self.add_line(f'assign: {", ".join(res_ids)}', indent + 1)
        
        c_obj = task.getConstraintType()
        c_date = self.format_date(task.getConstraintDate())
        if c_obj and int(c_obj.getValue()) != 0:
            cv = int(c_obj.getValue())
            if not c_date:
                c_date = self.format_date(task.getStart()) if cv in [2,4,5] else self.format_date(task.getFinish())
            
            mapping = {2: 'must_start_on', 4: 'start_no_earlier_than', 5: 'start_no_later_than',
                       3: 'must_finish_on', 6: 'finish_no_earlier_than', 7: 'finish_no_later_than'}
            if cv in mapping and c_date:
                self.add_line(f'{mapping[cv]}: {c_date}', indent + 1)

        notes = str(task.getNotes()) if task.getNotes() else ""
        if notes:
            notes_clean = notes.replace('\n', ' ').replace('\r', '').replace('"', '\\"').strip()
            if notes_clean:
                self.add_line(f'summary: "{notes_clean}"', indent + 1)

        if is_container:
            for child in children:
                self.write_task_recursive(child, indent + 1)
        
        self.add_line('}', indent)

# Note: We import Java classes dynamically to avoid errors if JVM is not started,
# but we define them here for easier mocking in tests.
def get_reader():
    from org.mpxj.reader import UniversalProjectReader
    return UniversalProjectReader()

def get_mspdi_writer():
    from org.mpxj.mspdi import MSPDIWriter
    return MSPDIWriter()

def convert_project(input_file: str, output_file: str) -> bool:
    if not jpype.isJVMStarted():
        classpath = [os.path.join(mpxj.mpxj_dir, f) for f in mpxj.filenames]
        jpype.startJVM("-Djava.awt.headless=true", classpath=classpath)
    
    try:
        print(f"Reading: {input_file}")
        reader = get_reader()
        project = reader.read(input_file)
        
        ext = Path(output_file).suffix.lower()
        print(f"Writing: {output_file}")
        
        if ext == '.proj':
            UTF8ProjWriter(project).write(output_file)
        else:
            get_mspdi_writer().write(project, output_file)
        print("Conversion successful!")
        return True
    except Exception as e:
        print(f"Error: {e}")
        import traceback
        traceback.print_exc()
        return False
    finally:
        if jpype.isJVMStarted(): jpype.shutdownJVM()

def main():
    if len(sys.argv) < 2:
        print("Usage: python mpp_to_proj.py <input.mpp> [output.proj|output.xml]")
        sys.exit(1)
    
    input_file = sys.argv[1]
    output_file = sys.argv[2] if len(sys.argv) >= 3 else str(Path(input_file).with_suffix('.proj'))
    
    if not os.path.exists(input_file):
        print(f"Error: Input file not found: {input_file}")
        sys.exit(1)
        
    Path(output_file).parent.mkdir(parents=True, exist_ok=True)
    if convert_project(input_file, output_file):
        sys.exit(0)
    sys.exit(1)

if __name__ == "__main__":
    main()
