import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.scalar.Scalar;
import java.util.*;
public class InputActionPushUsersEvidence extends GhidraScript {
 public void run()throws Exception{
  Listing l=currentProgram.getListing();
  Map<Function,Set<Long>> m=new LinkedHashMap<>();
  InstructionIterator it=l.getInstructions(true);
  while(it.hasNext()){
   Instruction ins=it.next();
   if(!"PUSH".equals(ins.getMnemonicString()))continue;
   Function f=getFunctionContaining(ins.getAddress()); if(f==null)continue;
   for(Object o:ins.getOpObjects(0)){
    if(o instanceof Scalar){
     long v=((Scalar)o).getUnsignedValue();
     if(v==0x25||v==0x2a||v==0x2b||v==0x2c||v==0x2d||v==0x2e||v==0x2f)
      m.computeIfAbsent(f,k->new LinkedHashSet<>()).add(v);
    }
   }
  }
  for(Map.Entry<Function,Set<Long>> e:m.entrySet()){
   Set<Long>s=e.getValue();
   if(s.contains(0x2cL)||s.contains(0x2dL))
    println(e.getKey().getEntryPoint()+" "+e.getKey().getName()+" ids="+s);
  }
 }
}