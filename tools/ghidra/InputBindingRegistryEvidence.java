import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;
import java.util.*;
public class InputBindingRegistryEvidence extends GhidraScript {
  DecompInterface d=new DecompInterface();
  void dec(Function f)throws Exception{
    if(f==null)return;
    println("\n=== "+f.getName()+" @ "+f.getEntryPoint()+" ===");
    DecompileResults r=d.decompileFunction(f,60,monitor);
    if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
  }
  void dec(long x)throws Exception{dec(getFunctionContaining(toAddr(x)));}
  void callers(long x,int max)throws Exception{
    println(String.format("\n=== CALLERS 0x%08X ===",x));
    LinkedHashSet<Function> fs=new LinkedHashSet<>();
    ReferenceIterator it=currentProgram.getReferenceManager().getReferencesTo(toAddr(x));
    while(it.hasNext()){
      Reference r=it.next();
      Function f=getFunctionContaining(r.getFromAddress());
      if(f!=null)fs.add(f);
    }
    int n=0;for(Function f:fs){println("CALLER "+f.getName()+" @ "+f.getEntryPoint());if(n++<max)dec(f);}
  }
  public void run()throws Exception{
    d.openProgram(currentProgram);
    dec(0x00508508L);
    dec(0x00507e11L);
    dec(0x00507cf8L);
    callers(0x00508519L,16);
    callers(0x00508625L,16);
    callers(0x00507e11L,12);
    d.dispose();
  }
}