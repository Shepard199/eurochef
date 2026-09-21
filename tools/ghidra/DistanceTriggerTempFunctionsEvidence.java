import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;

public class DistanceTriggerTempFunctionsEvidence extends GhidraScript {
  private void recover(long raw, String name, DecompInterface d) throws Exception {
    Address a=toAddr(raw); disassemble(a); Function f=getFunctionAt(a);
    if(f==null) f=createFunction(a,name);
    println(String.format("\n=== 0x%08X %s ===",raw,f==null?"<missing>":f.getName()));
    if(f==null)return;
    DecompileResults r=d.decompileFunction(f,90,monitor);
    if(r.decompileCompleted()&&r.getDecompiledFunction()!=null) println(r.getDecompiledFunction().getC());
  }
  @Override public void run() throws Exception {
    DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
    recover(0x0048D050L,"tmp_Distance_08",d);
    recover(0x0048D070L,"tmp_Distance_60",d);
    recover(0x0048D1C0L,"tmp_Distance_78",d);
    recover(0x0048D2B0L,"tmp_Distance_F0",d);
    recover(0x0048D2D0L,"tmp_Distance_6C",d);
    recover(0x0048D2F0L,"tmp_Distance_B8",d);
    recover(0x0048D490L,"tmp_Distance_FC",d);
    recover(0x0048FA50L,"tmp_Distance_Dtor",d);
    recover(0x0048FA60L,"tmp_Distance_F8",d);
    recover(0x0048FA70L,"tmp_Distance_7C",d);
    recover(0x0048FAF0L,"tmp_Distance_104",d);
    d.dispose();
  }
}