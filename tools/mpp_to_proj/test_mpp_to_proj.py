import sys
import os
from unittest.mock import MagicMock, patch
import pytest
from pathlib import Path

# Import from the local module (standalone companion tool)
from mpp_to_proj import UTF8ProjWriter, convert_project, main

class TestUTF8ProjWriter:
    @pytest.fixture
    def mock_project(self):
        project = MagicMock()
        props = MagicMock()
        project.getProjectProperties.return_value = props
        props.getProjectTitle.return_value = "Test Project"
        props.getStartDate.return_value = None
        props.getFinishDate.return_value = None
        return project

    @pytest.fixture
    def writer(self, mock_project):
        return UTF8ProjWriter(mock_project)

    def test_sanitize_id(self, writer):
        assert writer.sanitize_id("Task Name") == "task_name"
        assert writer.sanitize_id("Task+Name!") == "task_name"
        assert writer.sanitize_id("") == "res_" + str(hash(""))

    def test_sanitize_task_id(self, writer):
        assert writer.sanitize_task_id(123) == "task_123"

    def test_format_date(self, writer):
        assert writer.format_date(None) is None
        mock_date = MagicMock()
        mock_date.__str__.return_value = "2025-01-01T10:00:00"
        assert writer.format_date(mock_date) == "2025-01-01"
        
        # Test error handling
        bad_date = MagicMock()
        bad_date.__str__.side_effect = Exception("error")
        assert writer.format_date(bad_date) is None

    def test_add_line(self, writer):
        writer.add_line("test", indent=1)
        assert writer.lines[-1] == "    test"

    def test_generate_calendar(self, writer):
        writer.generate_calendar()
        assert any('calendar "standard" {' in line for line in writer.lines)
        assert any('holiday "Christmas"' in line for line in writer.lines)

    def test_generate_project_block(self, writer):
        writer.generate_project_block()
        assert any('project "Test Project" {' in line for line in writer.lines)
        assert any('currency: EUR' in line for line in writer.lines)
        
        # Test with dates
        writer.lines = []
        writer.project.getProjectProperties().getStartDate.return_value = "2025-01-01"
        writer.project.getProjectProperties().getFinishDate.return_value = "2025-12-31"
        writer.generate_project_block()
        output = "\n".join(writer.lines)
        assert "start: 2025-01-01" in output
        assert "end: 2025-12-31" in output

    def test_generate_resources(self, writer):
        res1 = MagicMock()
        res1.getUniqueID.return_value = 1
        res1.getName.return_value = "Alice"
        
        # Duplicate name test
        res2 = MagicMock()
        res2.getUniqueID.return_value = 2
        res2.getName.return_value = "Alice"
        
        # Null UID test
        res3 = MagicMock()
        res3.getUniqueID.return_value = None
        
        # Null Name test
        res4 = MagicMock()
        res4.getUniqueID.return_value = 4
        res4.getName.return_value = None
        
        writer.project.getResources.return_value = [res1, res2, res3, res4]
        
        writer.generate_resources()
        assert writer.resource_map[1] == "alice"
        assert writer.resource_map[2] == "alice_1"
        assert any('resource alice "Alice" {' in line for line in writer.lines)
        assert any('resource alice_1 "Alice" {' in line for line in writer.lines)

    def test_get_full_id(self, writer):
        t1 = MagicMock()
        t1.getUniqueID.return_value = 1
        t1.getID.return_value = 1
        
        t2 = MagicMock()
        t2.getUniqueID.return_value = 2
        t2.getID.return_value = 2
        t2.getParentTask.return_value = t1
        
        t1.getParentTask.return_value = None
        
        assert writer.get_full_id(t2) == "task_1.task_2"
        
        # Test stop at ID 0
        t0 = MagicMock()
        t0.getUniqueID.return_value = 0
        t0.getID.return_value = 0
        t2.getParentTask.return_value = t0
        assert writer.get_full_id(t2) == "task_2"

    def test_write_task_recursive_leaf(self, writer):
        task = MagicMock()
        task.getUniqueID.return_value = 10
        task.getName.return_value = 'Task "Quote"'
        task.getWBS.return_value = "1.1"
        task.getChildTasks.return_value = None
        task.getMilestone.return_value = False
        
        duration = MagicMock()
        duration.getDuration.return_value = 5.0
        task.getDuration.return_value = duration
        task.getPredecessors.return_value = []
        task.getResourceAssignments.return_value = []
        task.getConstraintType.return_value = None
        task.getNotes.return_value = "Some\nnotes with \"quotes\""
        
        writer.write_task_recursive(task, 0)
        
        output = "\n".join(writer.lines)
        assert 'task task_10 "[1.1] Task \\"Quote\\"" {' in output
        assert 'duration: 5d' in output
        assert 'summary: "Some notes with \\"quotes\\""' in output
        assert '}' in output

    def test_write_task_recursive_container(self, writer):
        parent = MagicMock()
        parent.getUniqueID.return_value = 1
        parent.getName.return_value = "Parent"
        parent.getWBS.return_value = "1"
        
        child = MagicMock()
        child.getUniqueID.return_value = 2
        child.getName.return_value = "Child"
        child.getWBS.return_value = "1.1"
        child.getChildTasks.return_value = None
        child.getMilestone.return_value = True
        child.getPredecessors.return_value = []
        child.getResourceAssignments.return_value = []
        child.getConstraintType.return_value = None
        child.getNotes.return_value = None
        
        mock_children = MagicMock()
        mock_children.isEmpty.return_value = False
        mock_children.__iter__.return_value = [child]
        parent.getChildTasks.return_value = mock_children
        
        parent.getPredecessors.return_value = []
        parent.getResourceAssignments.return_value = []
        parent.getConstraintType.return_value = None
        parent.getNotes.return_value = None
        
        writer.write_task_recursive(parent, 0)
        output = "\n".join(writer.lines)
        assert 'task task_1 "[1] Parent" {' in output
        assert 'task task_2 "[1.1] Child" {' in output

    def test_write_task_recursive_milestone(self, writer):
        task = MagicMock()
        task.getUniqueID.return_value = 20
        task.getMilestone.return_value = True
        task.getChildTasks.return_value = None
        task.getPredecessors.return_value = []
        task.getResourceAssignments.return_value = []
        task.getConstraintType.return_value = None
        task.getNotes.return_value = None

        writer.write_task_recursive(task, 0)
        assert any('milestone: true' in line for line in writer.lines)

    def test_write_task_recursive_with_effort(self, writer):
        """Test that non-zero Work value produces effort: line."""
        task = MagicMock()
        task.getUniqueID.return_value = 60
        task.getName.return_value = "Task With Effort"
        task.getWBS.return_value = "2.1"
        task.getChildTasks.return_value = None
        task.getMilestone.return_value = False
        task.getPredecessors.return_value = []
        task.getResourceAssignments.return_value = []
        task.getConstraintType.return_value = None
        task.getNotes.return_value = None

        # Mock duration
        duration = MagicMock()
        duration.getDuration.return_value = 3.0
        task.getDuration.return_value = duration

        # Mock work (effort)
        work = MagicMock()
        work.getDuration.return_value = 5.0
        task.getWork.return_value = work

        writer.write_task_recursive(task, 0)
        output = "\n".join(writer.lines)
        assert 'effort: 5d' in output

    def test_write_task_recursive_zero_work(self, writer):
        """Test that zero Work value does not produce effort: line."""
        task = MagicMock()
        task.getUniqueID.return_value = 61
        task.getName.return_value = "Task Zero Work"
        task.getWBS.return_value = "2.2"
        task.getChildTasks.return_value = None
        task.getMilestone.return_value = False
        task.getPredecessors.return_value = []
        task.getResourceAssignments.return_value = []
        task.getConstraintType.return_value = None
        task.getNotes.return_value = None

        # Mock duration
        duration = MagicMock()
        duration.getDuration.return_value = 4.0
        task.getDuration.return_value = duration

        # Mock work = 0
        work = MagicMock()
        work.getDuration.return_value = 0.0
        task.getWork.return_value = work

        writer.write_task_recursive(task, 0)
        output = "\n".join(writer.lines)
        assert 'duration: 4d' in output
        assert 'effort:' not in output

    def test_write_task_recursive_missing_work(self, writer):
        """Test that missing Work (None) does not produce effort: line."""
        task = MagicMock()
        task.getUniqueID.return_value = 62
        task.getName.return_value = "Task Missing Work"
        task.getWBS.return_value = "2.3"
        task.getChildTasks.return_value = None
        task.getMilestone.return_value = False
        task.getPredecessors.return_value = []
        task.getResourceAssignments.return_value = []
        task.getConstraintType.return_value = None
        task.getNotes.return_value = None

        # Mock duration
        duration = MagicMock()
        duration.getDuration.return_value = 6.0
        task.getDuration.return_value = duration

        # Work is None (missing)
        task.getWork.return_value = None

        writer.write_task_recursive(task, 0)
        output = "\n".join(writer.lines)
        assert 'duration: 6d' in output
        assert 'effort:' not in output

    def test_write_task_recursive_effort_and_duration(self, writer):
        """Test that both duration and effort are written when both are present."""
        task = MagicMock()
        task.getUniqueID.return_value = 63
        task.getName.return_value = "Task Both"
        task.getWBS.return_value = "2.4"
        task.getChildTasks.return_value = None
        task.getMilestone.return_value = False
        task.getPredecessors.return_value = []
        task.getResourceAssignments.return_value = []
        task.getConstraintType.return_value = None
        task.getNotes.return_value = None

        # Mock duration = 10 days
        duration = MagicMock()
        duration.getDuration.return_value = 10.0
        task.getDuration.return_value = duration

        # Mock work = 8 days (different from duration)
        work = MagicMock()
        work.getDuration.return_value = 8.0
        task.getWork.return_value = work

        writer.write_task_recursive(task, 0)
        output = "\n".join(writer.lines)
        assert 'duration: 10d' in output
        assert 'effort: 8d' in output

    def test_write_task_recursive_effort_fractional(self, writer):
        """Test that fractional effort values are preserved."""
        task = MagicMock()
        task.getUniqueID.return_value = 64
        task.getName.return_value = "Task Fractional"
        task.getWBS.return_value = "2.5"
        task.getChildTasks.return_value = None
        task.getMilestone.return_value = False
        task.getPredecessors.return_value = []
        task.getResourceAssignments.return_value = []
        task.getConstraintType.return_value = None
        task.getNotes.return_value = None

        # No duration
        task.getDuration.return_value = None

        # Mock work = 2.5 days (fractional)
        work = MagicMock()
        work.getDuration.return_value = 2.5
        task.getWork.return_value = work

        writer.write_task_recursive(task, 0)
        output = "\n".join(writer.lines)
        assert 'effort: 2.5d' in output

    def test_write_task_recursive_with_dependencies(self, writer):
        task = MagicMock()
        task.getUniqueID.return_value = 30
        task.getChildTasks.return_value = None
        
        pred_task = MagicMock()
        pred_task.getUniqueID.return_value = 5
        pred_task.getID.return_value = 5
        pred_task.getParentTask.return_value = None
        
        rel = MagicMock()
        rel.getPredecessorTask.return_value = pred_task
        rel.getType.return_value = "START_START"
        lag = MagicMock()
        lag.getDuration.return_value = 2.0
        rel.getLag.return_value = lag
        
        task.getPredecessors.return_value = [rel]
        task.getResourceAssignments.return_value = []
        task.getConstraintType.return_value = None
        task.getNotes.return_value = None
        
        writer.write_task_recursive(task, 0)
        output = "\n".join(writer.lines)
        assert 'depends: task_5 SS +2d' in output

    def test_write_task_recursive_with_assignments(self, writer):
        task = MagicMock()
        task.getUniqueID.return_value = 40
        task.getChildTasks.return_value = None
        
        res = MagicMock()
        res.getUniqueID.return_value = 100
        writer.resource_map[100] = "bob"
        
        assign = MagicMock()
        assign.getResource.return_value = res
        task.getResourceAssignments.return_value = [assign]
        task.getPredecessors.return_value = []
        task.getConstraintType.return_value = None
        task.getNotes.return_value = None
        
        writer.write_task_recursive(task, 0)
        assert any('assign: bob' in line for line in writer.lines)

    def test_write_task_recursive_all_constraints(self, writer):
        for cv, expected_keyword in [
            (2, 'must_start_on'),
            (4, 'start_no_earlier_than'),
            (5, 'start_no_later_than'),
            (3, 'must_finish_on'),
            (6, 'finish_no_earlier_than'),
            (7, 'finish_no_later_than')
        ]:
            writer.lines = []
            task = MagicMock()
            task.getUniqueID.return_value = 50
            task.getChildTasks.return_value = None
            task.getNotes.return_value = None
            task.getPredecessors.return_value = []
            task.getResourceAssignments.return_value = []
            
            c_type = MagicMock()
            c_type.getValue.return_value = cv
            task.getConstraintType.return_value = c_type
            
            # Case with constraint date
            c_date = MagicMock()
            c_date.__str__.return_value = "2025-05-05"
            task.getConstraintDate.return_value = c_date
            
            writer.write_task_recursive(task, 0)
            assert any(f'{expected_keyword}: 2025-05-05' in line for line in writer.lines)
            
            # Case with missing constraint date (fallback)
            writer.lines = []
            task.getConstraintDate.return_value = None
            task.getStart.return_value = "2025-01-01"
            task.getFinish.return_value = "2025-12-31"
            writer.write_task_recursive(task, 0)
            if cv in [2,4,5]:
                assert any(f'{expected_keyword}: 2025-01-01' in line for line in writer.lines)
            else:
                assert any(f'{expected_keyword}: 2025-12-31' in line for line in writer.lines)

    def test_get_reader(self):
        with patch.dict(sys.modules, {'org': MagicMock(), 'org.mpxj': MagicMock(), 'org.mpxj.reader': MagicMock()}):
             from mpp_to_proj import get_reader
             assert get_reader() is not None

    def test_get_mspdi_writer(self):
        with patch.dict(sys.modules, {'org': MagicMock(), 'org.mpxj': MagicMock(), 'org.mpxj.mspdi': MagicMock()}):
             from mpp_to_proj import get_mspdi_writer
             assert get_mspdi_writer() is not None

    def test_write(self, writer):
        # Mock all internal calls
        with patch.object(writer, 'generate_project_block') as m1, \
             patch.object(writer, 'generate_calendar') as m2, \
             patch.object(writer, 'generate_resources') as m3, \
             patch.object(writer, 'generate_tasks') as m4:
            
            with patch("builtins.open", MagicMock()) as mock_open:
                writer.write("out.proj")
                assert m1.called
                assert m2.called
                assert m3.called
                assert m4.called
                mock_open.assert_called_with("out.proj", 'w', encoding='utf-8')

    def test_write_task_recursive_null_uid(self, writer):
        task = MagicMock()
        task.getUniqueID.return_value = None
        writer.write_task_recursive(task, 0)
        assert len(writer.lines) == 0

    def test_generate_tasks_unwrapping(self, writer):
        root = MagicMock()
        root.getID.return_value = 0
        child = MagicMock()
        child.getUniqueID.return_value = 1
        
        # Setup top level tasks list mock
        mock_child_tasks = MagicMock()
        mock_child_tasks.size.return_value = 1
        mock_child_tasks.get.return_value = child
        mock_child_tasks.__iter__.return_value = [child]
        root.getChildTasks.return_value = mock_child_tasks
        
        mock_top_tasks = MagicMock()
        mock_top_tasks.size.return_value = 1
        mock_top_tasks.get.return_value = root
        mock_top_tasks.__iter__.return_value = [root]
        writer.project.getChildTasks.return_value = mock_top_tasks
        
        with patch.object(writer, 'write_task_recursive') as mock_write:
            writer.generate_tasks()
            assert mock_write.called

    @patch("mpp_to_proj.get_reader")
    @patch("mpp_to_proj.get_mspdi_writer")
    @patch("mpp_to_proj.jpype")
    @patch("mpp_to_proj.UTF8ProjWriter")
    def test_convert_project(self, mock_writer_class, mock_jpype, mock_get_mspdi, mock_get_reader):
        # Case 1: JVM not started
        mock_jpype.isJVMStarted.side_effect = [False, True, True, True, True, True]
        mock_reader = mock_get_reader.return_value
        mock_project = MagicMock()
        mock_reader.read.return_value = mock_project
        
        # Test XML
        with patch("mpp_to_proj.mpxj") as mock_mpxj:
            mock_mpxj.mpxj_dir = "dir"
            mock_mpxj.filenames = ["f1"]
            convert_project("in.mpp", "out.xml")
            mock_jpype.startJVM.assert_called()
            mock_get_mspdi.return_value.write.assert_called()
        
        # Test PROJ
        convert_project("in.mpp", "out.proj")
        mock_writer_class.return_value.write.assert_called_with("out.proj")
        
        # Test Error handling
        mock_reader.read.side_effect = Exception("error")
        assert convert_project("in.mpp", "out.proj") is False

    @patch("mpp_to_proj.convert_project")
    @patch("mpp_to_proj.os.path.exists")
    def test_main(self, mock_exists, mock_convert):
        mock_exists.return_value = True
        mock_convert.return_value = True
        
        # Test valid run
        with patch.object(sys, 'argv', ['script.py', 'in.mpp', 'out.proj']):
            with patch("mpp_to_proj.Path") as mock_path:
                mock_path.return_value.parent.mkdir = MagicMock()
                with pytest.raises(SystemExit) as e:
                    main()
                assert e.value.code == 0
        
        # Test missing input file
        mock_exists.return_value = False
        with patch.object(sys, 'argv', ['script.py', 'in.mpp']):
            with pytest.raises(SystemExit) as e:
                main()
            assert e.value.code == 1
            
        # Test usage help
        with patch.object(sys, 'argv', ['script.py']):
            with pytest.raises(SystemExit) as e:
                main()
            assert e.value.code == 1

        # Test failed conversion
        mock_exists.return_value = True
        mock_convert.return_value = False
        with patch.object(sys, 'argv', ['script.py', 'in.mpp']):
            with pytest.raises(SystemExit) as e:
                main()
            assert e.value.code == 1
